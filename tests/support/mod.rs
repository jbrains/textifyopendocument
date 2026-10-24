/*
 * textifyopendocument: a fast converter from OpenDocument to plain text.
 *
 * Copyright (c) 2026 jbrains
 *
 * This program is free software; you can redistribute it and/or
 * modify it under the terms of the GNU General Public License,
 * version 2 as published by the Free Software Foundation
 */

//! Shared helpers for integration tests: build ODF fixtures in code and
//! write them to temp files. Kept dependency-free.

use std::io::{Cursor, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Build a minimal zip archive with the given entries.
pub fn minimal_zip(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut zw = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in entries {
        zw.start_file(*name, SimpleFileOptions::default()).unwrap();
        zw.write_all(data).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

/// Wrap `body` in a full ODF content.xml document with the namespaces the
/// converter cares about.
pub fn content_xml(body: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <office:document-content\n\
         \x20xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\"\n\
         \x20xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\"\n\
         \x20xmlns:table=\"urn:oasis:names:tc:opendocument:xmlns:table:1.0\"\n\
         \x20xmlns:draw=\"urn:oasis:names:tc:opendocument:xmlns:drawing:1.0\">\n\
         <office:body>{}</office:body>\n\
         </office:document-content>",
        body
    )
}

/// Write `bytes` to a unique temp file and return its path.
pub fn write_temp_file(name: &str, bytes: &[u8]) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!(
        "tiod-integration-{}-{}-{}",
        std::process::id(),
        n,
        name
    ));
    std::fs::write(&path, bytes).unwrap();
    path
}
