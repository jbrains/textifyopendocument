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
use encoding::{detect_codeset, resolve_encoding, transcode, EncodingError};
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
    let target = match &config.encoding {
        Some(label) => resolve_encoding(label)?,
        None => detect_codeset(),
    };

    let mut reader = source::open_content_xml(config)?;

    if config.raw {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        out.write_all(&bytes)?;
        return Ok(());
    }

    let emit_cfg = EmitConfig {
        width: (config.width != -1).then_some(config.width as usize),
        limit_chars: config.limit_chars,
        deadline: config
            .limit_time_ms
            .map(|ms| Instant::now() + Duration::from_millis(ms)),
        subst: config.subst.clone(),
        encoding: target,
    };
    let mut emitter = Emitter::new(Vec::new(), emit_cfg);
    convert::convert(&mut quick_xml::Reader::from_reader(reader), &mut emitter)?;
    let utf8 = emitter.finish()?;

    let bytes = if target == encoding_rs::UTF_8 {
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
    Ok(())
}
