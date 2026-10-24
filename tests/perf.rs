mod support;

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_textifyopendocument"))
}

fn big_ods(rows: usize, cols: usize) -> PathBuf {
    let mut body = String::from("<table:table>");
    for r in 0..rows {
        body.push_str("<table:table-row>");
        for c in 0..cols {
            body.push_str(&format!(
                "<table:table-cell><text:p>cell {r}-{c}</text:p></table:table-cell>"
            ));
        }
        body.push_str("</table:table-row>");
    }
    body.push_str("</table:table>");
    let xml = support::content_xml(&body);
    let zip = support::minimal_zip(&[("content.xml", xml.into_bytes())]);
    support::write_temp_file("perf-big.ods", &zip)
}

#[test]
#[ignore]
fn big_spreadsheet_full_mode_and_preview() {
    let path = big_ods(20_000, 4);

    let started = Instant::now();
    let full = bin().arg(&path).output().unwrap();
    let full_elapsed = started.elapsed();
    assert!(full.status.success());
    assert!(
        full_elapsed < Duration::from_secs(60),
        "full mode took {:?}",
        full_elapsed
    );
    let full_text = String::from_utf8_lossy(&full.stdout);
    assert!(
        full_text.contains("cell 19999-3"),
        "full output missing last cell"
    );

    let started = Instant::now();
    let preview = bin()
        .arg("--limit-chars=5000")
        .arg(&path)
        .output()
        .unwrap();
    let preview_elapsed = started.elapsed();
    assert!(preview.status.success());
    assert!(
        preview_elapsed < Duration::from_secs(10),
        "preview took {:?}",
        preview_elapsed
    );
    let preview_text = String::from_utf8_lossy(&preview.stdout);
    assert!(
        preview_text.chars().count() <= 5000,
        "preview had {} chars",
        preview_text.chars().count()
    );
}
