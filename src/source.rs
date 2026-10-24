/*
 * textifyopendocument: a fast converter from OpenDocument to plain text.
 *
 * Copyright (c) 2026 jbrains
 *
 * This program is free software; you can redistribute it and/or
 * modify it under the terms of the GNU General Public License,
 * version 2 as published by the Free Software Foundation
 */

use std::io::BufRead;
use std::io::BufReader;

use crate::args::Config;

#[derive(Debug)]
pub enum SourceError {
    NotOpenDocument(String), // file not a zip, or zip has no content.xml
    Io(std::io::Error),
}

impl From<std::io::Error> for SourceError {
    fn from(e: std::io::Error) -> Self {
        SourceError::Io(e)
    }
}

pub fn open_content_xml(config: &Config) -> Result<Box<dyn BufRead>, SourceError> {
    let file = std::fs::File::open(&config.input)?;

    if config.raw_input {
        return Ok(Box::new(BufReader::new(file)));
    }

    let mut archive = zip::ZipArchive::new(file).map_err(|e| match e {
        zip::result::ZipError::FileNotFound | zip::result::ZipError::InvalidArchive(_) => {
            SourceError::NotOpenDocument(config.input.display().to_string())
        }
        other => SourceError::Io(other.into()),
    })?;

    let mut entry = archive.by_name("content.xml").map_err(|e| match e {
        zip::result::ZipError::FileNotFound => {
            SourceError::NotOpenDocument(config.input.display().to_string())
        }
        other => SourceError::Io(other.into()),
    })?;

    // Buffer the entry: ZipFile borrows the archive, so it cannot outlive it
    // as a Box<dyn BufRead>. Decompression still happens lazily via zip;
    // parsing still streams over the buffer.
    let mut bytes = Vec::new();
    use std::io::Read;
    entry.read_to_end(&mut bytes)?;
    Ok(Box::new(BufReader::new(std::io::Cursor::new(bytes))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::SubstMode;
    use std::fs;
    use std::io::{Cursor, Read, Write};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_path(name: &str) -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!(
            "tiod-test-{}-{}-{}",
            std::process::id(),
            n,
            name
        ))
    }

    fn write_bytes(name: &str, bytes: &[u8]) -> PathBuf {
        let path = temp_path(name);
        fs::write(&path, bytes).unwrap();
        path
    }

    fn build_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut zw = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data) in entries {
            zw.start_file(*name, SimpleFileOptions::default()).unwrap();
            zw.write_all(data).unwrap();
        }
        zw.finish().unwrap().into_inner()
    }

    fn config(path: &PathBuf, raw_input: bool) -> Config {
        Config {
            input: path.clone(),
            raw: false,
            raw_input,
            width: 65,
            output: None,
            encoding: None,
            subst: SubstMode::None,
            limit_chars: None,
            limit_time_ms: None,
        }
    }

    fn read_all(mut r: Box<dyn BufRead>) -> String {
        let mut s = String::new();
        r.read_to_string(&mut s).unwrap();
        s
    }

    #[test]
    fn reads_content_xml_from_zip() {
        let path = write_bytes(
            "doc.odt",
            &build_zip(&[("content.xml", b"hello content")]),
        );
        let r = open_content_xml(&config(&path, false)).unwrap();
        assert_eq!(read_all(r), "hello content");
    }

    #[test]
    fn raw_input_reads_file_directly() {
        let path = write_bytes("doc.fodt", b"raw xml bytes");
        let r = open_content_xml(&config(&path, true)).unwrap();
        assert_eq!(read_all(r), "raw xml bytes");
    }

    #[test]
    fn missing_content_xml_is_not_open_document() {
        let path = write_bytes("no-content.odt", &build_zip(&[("mimetype", b"x")]));
        let err = match open_content_xml(&config(&path, false)) {
            Err(e) => e,
            Ok(_) => panic!("expected error"),
        };
        assert!(matches!(err, SourceError::NotOpenDocument(_)));
    }

    #[test]
    fn non_zip_is_not_open_document() {
        let path = write_bytes("plain.txt", b"this is not a zip");
        let err = match open_content_xml(&config(&path, false)) {
            Err(e) => e,
            Ok(_) => panic!("expected error"),
        };
        assert!(matches!(err, SourceError::NotOpenDocument(_)));
    }
}
