/*
 * textifyopendocument: a fast converter from OpenDocument to plain text.
 *
 * Copyright (c) 2026 jbrains
 *
 * This program is free software; you can redistribute it and/or
 * modify it under the terms of the GNU General Public License,
 * version 2 as published by the Free Software Foundation
 */

mod support;

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_textifyopendocument"))
}

#[test]
fn version_flag_prints_name_and_exits_zero() {
    let out = bin().arg("--version").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains(concat!("textifyopendocument ", env!("CARGO_PKG_VERSION"))),
        "--version printed {:?}, expected the crate version",
        stdout
    );
}

#[test]
fn converts_odt_fixture() {
    let xml = support::content_xml("<text:p>Hello</text:p>");
    let zip = support::minimal_zip(&[("content.xml", xml.into_bytes())]);
    let path = support::write_temp_file("cli-golden.odt", &zip);
    let out = bin().arg(&path).output().unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "Hello\n");
}

#[test]
fn garbage_file_exits_nonzero_with_filename() {
    let path = support::write_temp_file("cli-garbage.bin", b"definitely not a zip");
    let out = bin().arg(&path).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("cli-garbage.bin"),
        "stderr should mention the filename, got: {}",
        stderr
    );
}

#[test]
fn limit_chars_truncates() {
    let xml = support::content_xml("<text:p>abcdefgh</text:p>");
    let zip = support::minimal_zip(&[("content.xml", xml.into_bytes())]);
    let path = support::write_temp_file("cli-limit.odt", &zip);
    let out = bin().arg("--limit-chars=5").arg(&path).output().unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "abcde");
}

#[test]
fn limit_time_truncates() {
    let mut body = String::from("<table:table>");
    for r in 0..20_000 {
        body.push_str("<table:table-row>");
        for c in 0..4 {
            body.push_str(&format!(
                "<table:table-cell><text:p>cell {r}-{c}</text:p></table:table-cell>"
            ));
        }
        body.push_str("</table:table-row>");
    }
    body.push_str("</table:table>");
    let xml = support::content_xml(&body);
    let zip = support::minimal_zip(&[("content.xml", xml.into_bytes())]);
    let path = support::write_temp_file("cli-limit-time.ods", &zip);

    let started = std::time::Instant::now();
    let out = bin().arg("--limit-time=1").arg(&path).output().unwrap();
    let elapsed = started.elapsed();
    assert!(out.status.success());
    assert!(
        elapsed < std::time::Duration::from_secs(5),
        "took {:?}",
        elapsed
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Full output is ~1M chars; release builds parse ~12k chars within the
    // 1 ms window, debug builds ~1.7k. Any value comfortably below the full
    // length proves truncation without depending on build speed.
    assert!(
        stdout.chars().count() < 100_000,
        "expected early truncation, got {} chars",
        stdout.chars().count()
    );
}

#[test]
fn malformed_xml_exits_nonzero_but_keeps_partial_output() {
    let xml = support::content_xml("<text:p>Hello</text:p><text:p>oops");
    let zip = support::minimal_zip(&[("content.xml", xml.into_bytes())]);
    let path = support::write_temp_file("cli-malformed.odt", &zip);
    let out = bin().arg(&path).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Hello"),
        "partial output lost, got: {:?}",
        stdout
    );
}

#[test]
fn truncated_xml_exits_nonzero() {
    let xml = "<office:document><office:body><text:p>Hello</text:p><text:p>partial";
    let zip = support::minimal_zip(&[("content.xml", xml.as_bytes().to_vec())]);
    let path = support::write_temp_file("cli-truncated.odt", &zip);
    let out = bin().arg(&path).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn unknown_encoding_warns_and_uses_utf8() {
    let xml = support::content_xml("<text:p>€ä☃</text:p>");
    let zip = support::minimal_zip(&[("content.xml", xml.into_bytes())]);
    let path = support::write_temp_file("cli-unknown-encoding.odt", &zip);
    let out = bin().arg("--encoding=not-a-codeset").arg(&path).output().unwrap();
    assert!(out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("warning"), "stderr: {}", stderr);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "€ä☃\n");
}

#[test]
fn ascii_encoding_substitutes_look_alikes() {
    let xml = support::content_xml("<text:p>€ä☃</text:p>");
    let zip = support::minimal_zip(&[("content.xml", xml.into_bytes())]);
    let path = support::write_temp_file("cli-ascii.odt", &zip);
    let out = bin().arg("--encoding=ascii").arg(&path).output().unwrap();
    assert!(out.status.success());
    assert_eq!(out.stdout, b"EURae?\n");
}

#[test]
fn huge_text_c_attribute_does_not_panic() {
    let xml = support::content_xml(
        "<text:p>A<text:s text:c=\"18446744073709551615\"/>B</text:p>",
    );
    let zip = support::minimal_zip(&[("content.xml", xml.into_bytes())]);
    let path = support::write_temp_file("cli-huge-c.odt", &zip);
    let out = bin().arg("--limit-chars=10").arg(&path).output().unwrap();
    assert!(out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stdout).chars().count() <= 10
    );
}
