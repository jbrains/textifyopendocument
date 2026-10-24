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
