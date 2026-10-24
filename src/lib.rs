/*
 * textifyopendocument: a fast converter from OpenDocument to plain text.
 *
 * Copyright (c) 2026 jbrains
 *
 * This program is free software; you can redistribute it and/or
 * modify it under the terms of the GNU General Public License,
 * version 2 as published by the Free Software Foundation
 */

pub mod args;
pub mod convert;
pub mod emit;
pub mod encoding;
pub mod source;
pub mod subst;

use std::io::{Read, Write};
use std::time::{Duration, Instant};

pub use args::{Config, SubstMode};

use convert::ConvertError;
use emit::{EmitConfig, Emitter};
use encoding::{detect_codeset, resolve_encoding, transcode, Charset, EncodingError};
use source::SourceError;

#[derive(Debug)]
pub enum Error {
    Source(SourceError),
    Convert(ConvertError),
    Encoding(EncodingError),
    Io(std::io::Error),
}

impl From<SourceError> for Error {
    fn from(e: SourceError) -> Self {
        Error::Source(e)
    }
}

impl From<ConvertError> for Error {
    fn from(e: ConvertError) -> Self {
        Error::Convert(e)
    }
}

impl From<EncodingError> for Error {
    fn from(e: EncodingError) -> Self {
        Error::Encoding(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Source(SourceError::NotOpenDocument(name)) => {
                write!(f, "{}: not an OpenDocument file?", name)
            }
            Error::Source(SourceError::Io(e)) => write!(f, "{}", e),
            Error::Convert(ConvertError::Xml(e)) => write!(f, "invalid XML: {}", e),
            Error::Convert(ConvertError::Malformed(m)) => write!(f, "invalid XML: {}", m),
            Error::Convert(ConvertError::Truncated) => write!(f, "truncated XML"),
            Error::Convert(ConvertError::Io(e)) => write!(f, "{}", e),
            Error::Encoding(EncodingError::Unknown(label)) => {
                write!(f, "unknown encoding: {}", label)
            }
            Error::Io(e) => write!(f, "{}", e),
        }
    }
}

pub fn run(config: &Config) -> Result<(), Error> {
    let stdout = std::io::stdout();
    run_into(config, &mut stdout.lock())
}

pub fn run_into(config: &Config, out: &mut dyn Write) -> Result<(), Error> {
    // The time deadline starts before any file work so it bounds
    // opening and decompression too.
    let deadline = config
        .limit_time_ms
        .map(|ms| Instant::now() + Duration::from_millis(ms));

    let target = match &config.encoding {
        Some(label) => match resolve_encoding(label) {
            Ok(cs) => cs,
            Err(_) => {
                eprintln!(
                    "textifyopendocument: warning: unknown encoding {}, using UTF-8",
                    label
                );
                Charset::Utf8
            }
        },
        None => detect_codeset(),
    };

    let mut source = source::open_source(config)?;

    if config.raw {
        let mut bytes = Vec::new();
        source.content_xml()?.read_to_end(&mut bytes)?;
        out.write_all(&bytes)?;
        return Ok(());
    }

    let emit_cfg = EmitConfig {
        width: (config.width != -1).then_some(config.width as usize),
        limit_chars: config.limit_chars,
        deadline,
        subst: config.subst.clone(),
        encoding: target,
    };
    let mut emitter = Emitter::new(Vec::new(), emit_cfg);
    let convert_result = {
        let reader = source.content_xml()?;
        convert::convert(&mut quick_xml::Reader::from_reader(reader), &mut emitter)
    };
    let utf8 = emitter.finish()?;

    let bytes = if target == Charset::Utf8 {
        utf8
    } else {
        transcode(target, &utf8)
    };

    match &config.output {
        Some(path) => {
            let mut file = std::fs::File::create(path)?;
            file.write_all(&bytes)?;
        }
        None => out.write_all(&bytes)?,
    }

    // Text emitted before a conversion error stays emitted; report the
    // error only after writing it out.
    convert_result?;
    Ok(())
}
