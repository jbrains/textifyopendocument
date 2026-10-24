mod support;

use std::path::PathBuf;

use textifyopendocument::args::{Config, SubstMode};
use textifyopendocument::{run_into, Error};

fn cfg(input: PathBuf) -> Config {
    Config {
        input,
        raw: false,
        raw_input: false,
        width: 65,
        output: None,
        encoding: None,
        subst: SubstMode::Some,
        limit_chars: None,
        limit_time_ms: None,
    }
}

fn run_to_vec(config: Config) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    run_into(&config, &mut out)?;
    Ok(out)
}

fn fixture_odt(name: &str, body: &str) -> PathBuf {
    let xml = support::content_xml(body);
    let zip = support::minimal_zip(&[("content.xml", xml.into_bytes())]);
    support::write_temp_file(name, &zip)
}

#[test]
fn odt_paragraphs_headings() {
    let path = fixture_odt(
        "golden-paragraphs.odt",
        "<text:p>First</text:p>\
         <text:p>Second</text:p>\
         <text:h text:outline-level=\"1\">Title</text:h>\
         <text:h>Sub</text:h>",
    );
    let out = run_to_vec(cfg(path)).unwrap();
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "First\n\nSecond\n\nTitle\n=====\n\nSub\n---\n"
    );
}

#[test]
fn ods_cells_flow() {
    let path = fixture_odt(
        "golden-cells.ods",
        "<table:table>\
           <table:table-row>\
             <table:table-cell><text:p>r1c1</text:p></table:table-cell>\
             <table:table-cell><text:p>r1c2</text:p></table:table-cell>\
           </table:table-row>\
           <table:table-row>\
             <table:table-cell><text:p>r2c1</text:p></table:table-cell>\
             <table:table-cell><text:p>r2c2</text:p></table:table-cell>\
           </table:table-row>\
         </table:table>",
    );
    let out = run_to_vec(cfg(path)).unwrap();
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "r1c1\n\nr1c2\n\nr2c1\n\nr2c2\n"
    );
}

#[test]
fn odp_frame_text() {
    let path = fixture_odt(
        "golden-frame.odp",
        "<draw:frame draw:name=\"pic1\">\
           <draw:text-box><text:p>Caption</text:p></draw:text-box>\
           <draw:image/>\
         </draw:frame>",
    );
    let out = run_to_vec(cfg(path)).unwrap();
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "Caption\n\n[-- Image: pic1 --]\n"
    );
}

#[test]
fn raw_flag_prints_xml() {
    let xml = support::content_xml("<text:p>Hello</text:p>");
    let zip = support::minimal_zip(&[("content.xml", xml.clone().into_bytes())]);
    let path = support::write_temp_file("golden-raw.odt", &zip);
    let mut c = cfg(path);
    c.raw = true;
    let out = run_to_vec(c).unwrap();
    assert_eq!(String::from_utf8(out).unwrap(), xml);
}
