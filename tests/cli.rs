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
    assert!(stdout.contains("textifyopendocument"));
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
    assert!(
        stdout.chars().count() < 10_000,
        "expected early truncation, got {} chars",
        stdout.chars().count()
    );
}
