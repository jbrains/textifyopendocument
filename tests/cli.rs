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
