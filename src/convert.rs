/*
 * textifyopendocument: a fast converter from OpenDocument to plain text.
 *
 * Copyright (c) 2026 jbrains
 *
 * This program is free software; you can redistribute it and/or
 * modify it under the terms of the GNU General Public License,
 * version 2 as published by the Free Software Foundation
 */

use std::io::{BufRead, Write};

use quick_xml::events::Event;

use crate::emit::Emitter;

#[derive(Debug)]
pub enum ConvertError {
    Xml(quick_xml::Error),
    Io(std::io::Error),
}

pub fn convert<R: BufRead, W: Write>(
    reader: &mut quick_xml::Reader<R>,
    emitter: &mut Emitter<W>,
) -> Result<(), ConvertError> {
    let mut buf = Vec::new();
    let mut in_body = false;
    let mut skip_depth: usize = 0;
    let mut heading: Option<(String, u8)> = None;
    let mut frame_names: Vec<Option<String>> = Vec::new();

    loop {
        if emitter.limit_reached() {
            return Ok(());
        }
        let ev = reader.read_event_into(&mut buf).map_err(ConvertError::Xml)?;
        match ev {
            Event::Start(ref e) => {
                let name = e.name();
                if name.as_ref() == b"office:binary-data" {
                    if skip_depth > 0 {
                        skip_depth += 1;
                    } else {
                        skip_depth = 1;
                    }
                } else if skip_depth > 0 {
                    // inside a skipped subtree
                } else if name.as_ref() == b"office:body" {
                    in_body = true;
                } else if name.as_ref() == b"draw:frame" {
                    frame_names.push(attr_value(e, b"draw:name"));
                } else if name.as_ref() == b"draw:image" {
                    if in_body {
                        if let Some(Some(frame_name)) = frame_names.last() {
                            emitter
                                .emit_image_marker(frame_name)
                                .map_err(ConvertError::Io)?;
                        }
                    }
                } else if name.as_ref() == b"text:h" {
                    let level = heading_level(e);
                    heading = Some((String::new(), level));
                } else if name.as_ref() == b"text:s" {
                    emitter
                        .emit_spaces(space_count(e))
                        .map_err(ConvertError::Io)?;
                } else if name.as_ref() == b"text:tab" {
                    emitter.emit_spaces(2).map_err(ConvertError::Io)?;
                } else if name.as_ref() == b"text:line-break" {
                    emitter.emit_line_break().map_err(ConvertError::Io)?;
                }
            }
            Event::Empty(ref e) => {
                let name = e.name();
                if skip_depth > 0 {
                    // inside a skipped subtree
                } else if name.as_ref() == b"draw:image" {
                    if in_body {
                        if let Some(Some(frame_name)) = frame_names.last() {
                            emitter
                                .emit_image_marker(frame_name)
                                .map_err(ConvertError::Io)?;
                        }
                    }
                } else if name.as_ref() == b"text:h" {
                    emitter
                        .emit_heading(heading_level(e), "")
                        .map_err(ConvertError::Io)?;
                } else if name.as_ref() == b"text:s" {
                    emitter
                        .emit_spaces(space_count(e))
                        .map_err(ConvertError::Io)?;
                } else if name.as_ref() == b"text:tab" {
                    emitter.emit_spaces(2).map_err(ConvertError::Io)?;
                } else if name.as_ref() == b"text:line-break" {
                    emitter.emit_line_break().map_err(ConvertError::Io)?;
                } else if name.as_ref() == b"text:p" {
                    emitter
                        .emit_paragraph_break()
                        .map_err(ConvertError::Io)?;
                }
            }
            Event::End(ref e) => {
                let name = e.name();
                if name.as_ref() == b"office:binary-data" && skip_depth > 0 {
                    skip_depth -= 1;
                } else if skip_depth > 0 {
                    // inside a skipped subtree
                } else if name.as_ref() == b"office:body" {
                    in_body = false;
                } else if name.as_ref() == b"draw:frame" {
                    frame_names.pop();
                } else if name.as_ref() == b"text:p" {
                    emitter
                        .emit_paragraph_break()
                        .map_err(ConvertError::Io)?;
                } else if name.as_ref() == b"text:h" {
                    if let Some((text, level)) = heading.take() {
                        emitter
                            .emit_heading(level, &text)
                            .map_err(ConvertError::Io)?;
                    }
                }
            }
            Event::Text(t) => {
                if in_body && skip_depth == 0 {
                    let text = t.unescape().map_err(ConvertError::Xml)?;
                    if let Some((buf, _)) = heading.as_mut() {
                        buf.push_str(&text);
                    } else {
                        emitter.emit_text(&text).map_err(ConvertError::Io)?;
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(())
}

fn heading_level(e: &quick_xml::events::BytesStart) -> u8 {
    if attr_value(e, b"text:outline-level").as_deref() == Some("1") {
        1
    } else {
        2
    }
}

fn space_count(e: &quick_xml::events::BytesStart) -> usize {
    attr_value(e, b"text:c")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(1)
}

fn attr_value(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<String> {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == key {
            return attr.unescape_value().ok().map(|v| v.into_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::SubstMode;
    use crate::emit::{EmitConfig, Emitter};
    use std::io::Cursor;

    fn doc(body: &str) -> String {
        format!(
            "<office:document \
             xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
             xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\">\
             <office:body>{}</office:body></office:document>",
            body
        )
    }

    fn convert_str(xml: &str) -> Result<String, ConvertError> {
        let mut reader = quick_xml::Reader::from_reader(Cursor::new(xml.as_bytes()));
        let mut emitter = Emitter::new(
            Vec::new(),
            EmitConfig {
                width: None,
                limit_chars: None,
                deadline: None,
                subst: SubstMode::None,
                encoding: encoding_rs::UTF_8,
            },
        );
        convert(&mut reader, &mut emitter)?;
        let bytes = emitter.finish().map_err(ConvertError::Io)?;
        Ok(String::from_utf8(bytes).unwrap())
    }

    #[test]
    fn body_gating() {
        let xml = "<office:document><office:meta>meta text</office:meta>\
                   <office:body><text:p>Hello</text:p></office:body></office:document>";
        assert_eq!(convert_str(xml).unwrap(), "Hello\n");
    }

    #[test]
    fn paragraph_breaks() {
        assert_eq!(
            convert_str(&doc("<text:p>A</text:p><text:p>B</text:p>")).unwrap(),
            "A\n\nB\n"
        );
    }

    #[test]
    fn empty_document() {
        assert_eq!(
            convert_str("<office:document><office:body/></office:document>").unwrap(),
            ""
        );
    }

    #[test]
    fn binary_data_skipped() {
        let xml = doc("<text:p>A</text:p><office:binary-data>Zm9v</office:binary-data>");
        assert_eq!(convert_str(&xml).unwrap(), "A\n");
    }

    #[test]
    fn soft_page_break_skipped() {
        let xml = doc("<text:p>a<text:soft-page-break/>b</text:p>");
        assert_eq!(convert_str(&xml).unwrap(), "ab\n");
    }

    #[test]
    fn protected_spaces() {
        let xml = doc("<text:p>A<text:s text:c=\"3\"/>B</text:p>");
        assert_eq!(convert_str(&xml).unwrap(), "A   B\n");
    }

    #[test]
    fn protected_single_space() {
        let xml = doc("<text:p>A<text:s/>B</text:p>");
        assert_eq!(convert_str(&xml).unwrap(), "A B\n");
    }

    #[test]
    fn tab_and_line_break() {
        let xml = doc("<text:p>A<text:tab/>B<text:line-break/>C</text:p>");
        assert_eq!(convert_str(&xml).unwrap(), "A  B\nC\n");
    }

    #[test]
    fn headings() {
        let xml = doc("<text:h text:outline-level=\"1\">Title</text:h>");
        assert_eq!(convert_str(&xml).unwrap(), "Title\n=====\n");

        let xml = doc("<text:h><text:span>Sub</text:span></text:h>");
        assert_eq!(convert_str(&xml).unwrap(), "Sub\n---\n");
    }

    #[test]
    fn entities_decoded() {
        let xml = doc("<text:p>a &amp; b &#65; &#x20AC;</text:p>");
        assert_eq!(convert_str(&xml).unwrap(), "a & b A €\n");
    }

    #[test]
    fn malformed_xml_is_error() {
        assert!(matches!(
            convert_str("</unclosed>"),
            Err(ConvertError::Xml(_))
        ));
    }

    #[test]
    fn frame_text_flows() {
        let xml = doc(
            "<draw:frame draw:name=\"pic1\">\
             <draw:text-box><text:p>Caption</text:p></draw:text-box>\
             </draw:frame>",
        );
        assert_eq!(convert_str(&xml).unwrap(), "Caption\n");
    }

    #[test]
    fn image_marker_at_image() {
        let xml = doc(
            "<draw:frame draw:name=\"pic1\">\
             <draw:text-box><text:p>Caption</text:p></draw:text-box>\
             <draw:image/>\
             </draw:frame>",
        );
        assert_eq!(
            convert_str(&xml).unwrap(),
            "Caption\n\n[-- Image: pic1 --]\n"
        );
    }

    #[test]
    fn no_name_no_marker() {
        let xml = doc(
            "<draw:frame>\
             <draw:text-box><text:p>Caption</text:p></draw:text-box>\
             <draw:image/>\
             </draw:frame>",
        );
        assert_eq!(convert_str(&xml).unwrap(), "Caption\n");
    }

    #[test]
    fn nested_frames_inner_name_wins() {
        let xml = doc(
            "<draw:frame draw:name=\"outer\">\
             <draw:frame draw:name=\"inner\"><draw:image/></draw:frame>\
             </draw:frame>",
        );
        let out = convert_str(&xml).unwrap();
        assert!(out.contains("[-- Image: inner --]"), "got: {:?}", out);
        assert!(!out.contains("outer"), "got: {:?}", out);
    }

    #[test]
    fn frame_text_without_body_gating() {
        let xml = "<office:document>\
                   <draw:frame draw:name=\"x\"><draw:image/></draw:frame>\
                   <office:body/>\
                   </office:document>";
        assert_eq!(convert_str(xml).unwrap(), "");
    }
}
