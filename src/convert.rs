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
    Malformed(String), // attribute-level malformation etc.
    Truncated,         // input ended with unclosed elements
    Io(std::io::Error),
}

pub fn convert<R: BufRead, W: Write>(
    reader: &mut quick_xml::Reader<R>,
    emitter: &mut Emitter<W>,
) -> Result<(), ConvertError> {
    let mut buf = Vec::new();
    let mut in_body = false;
    let mut skip_depth: usize = 0;
    let mut element_depth: usize = 0;
    let mut heading: Option<(String, u8)> = None;
    let mut frame_names: Vec<Option<String>> = Vec::new();

    loop {
        emitter.check_limits();
        if emitter.limit_reached() {
            return Ok(());
        }
        let ev = reader.read_event_into(&mut buf).map_err(ConvertError::Xml)?;
        match ev {
            Event::Start(ref e) => {
                let name = e.name();
                let local = name.local_name().as_ref().to_vec();
                if local == b"binary-data" {
                    if skip_depth > 0 {
                        skip_depth += 1;
                    } else {
                        skip_depth = 1;
                    }
                    element_depth += 1;
                } else if skip_depth > 0 {
                    element_depth += 1;
                    // inside a skipped subtree
                } else if local == b"body" {
                    element_depth += 1;
                    in_body = true;
                } else if local == b"frame" {
                    element_depth += 1;
                    frame_names.push(attr_value(e, b"name")?);
                } else if local == b"image" {
                    element_depth += 1;
                    if in_body {
                        if let Some(Some(frame_name)) = frame_names.last() {
                            emitter
                                .emit_image_marker(frame_name)
                                .map_err(ConvertError::Io)?;
                        }
                    }
                } else if local == b"h" {
                    element_depth += 1;
                    if in_body {
                        let level = heading_level(e)?;
                        heading = Some((String::new(), level));
                    }
                } else if local == b"s" {
                    element_depth += 1;
                    if in_body && skip_depth == 0 {
                        let spaces = space_count(e)?;
                        if let Some((buf, _)) = heading.as_mut() {
                            buf.push_str(&" ".repeat(spaces.min(crate::emit::MAX_SPACES)));
                        } else {
                            emitter.emit_spaces(spaces).map_err(ConvertError::Io)?;
                        }
                    }
                } else if local == b"tab" {
                    element_depth += 1;
                    if in_body && skip_depth == 0 {
                        if let Some((buf, _)) = heading.as_mut() {
                            buf.push_str("  ");
                        } else {
                            emitter.emit_spaces(2).map_err(ConvertError::Io)?;
                        }
                    }
                } else if local == b"line-break" {
                    element_depth += 1;
                    if in_body && skip_depth == 0 {
                        if let Some((buf, _)) = heading.as_mut() {
                            buf.push('\n');
                        } else {
                            emitter.emit_line_break().map_err(ConvertError::Io)?;
                        }
                    }
                } else {
                    element_depth += 1;
                }
            }
            Event::Empty(ref e) => {
                let name = e.name();
                let local = name.local_name().as_ref().to_vec();
                if skip_depth > 0 {
                    // inside a skipped subtree
                } else if local == b"image" {
                    if in_body {
                        if let Some(Some(frame_name)) = frame_names.last() {
                            emitter
                                .emit_image_marker(frame_name)
                                .map_err(ConvertError::Io)?;
                        }
                    }
                } else if local == b"h" {
                    if in_body {
                        emitter
                            .emit_heading(heading_level(e)?, "")
                            .map_err(ConvertError::Io)?;
                    }
                } else if local == b"s" {
                    if in_body {
                        let spaces = space_count(e)?;
                        if let Some((buf, _)) = heading.as_mut() {
                            buf.push_str(&" ".repeat(spaces.min(crate::emit::MAX_SPACES)));
                        } else {
                            emitter.emit_spaces(spaces).map_err(ConvertError::Io)?;
                        }
                    }
                } else if local == b"tab" {
                    if in_body {
                        if let Some((buf, _)) = heading.as_mut() {
                            buf.push_str("  ");
                        } else {
                            emitter.emit_spaces(2).map_err(ConvertError::Io)?;
                        }
                    }
                } else if local == b"line-break" {
                    if in_body {
                        if let Some((buf, _)) = heading.as_mut() {
                            buf.push('\n');
                        } else {
                            emitter.emit_line_break().map_err(ConvertError::Io)?;
                        }
                    }
                } else if local == b"p" {
                    if in_body {
                        emitter
                            .emit_paragraph_break()
                            .map_err(ConvertError::Io)?;
                    }
                }
            }
            Event::End(ref e) => {
                let name = e.name();
                let local = name.local_name().as_ref().to_vec();
                element_depth = element_depth.saturating_sub(1);
                if local == b"binary-data" && skip_depth > 0 {
                    skip_depth -= 1;
                } else if skip_depth > 0 {
                    // inside a skipped subtree
                } else if local == b"body" {
                    in_body = false;
                } else if local == b"frame" {
                    frame_names.pop();
                } else if local == b"p" {
                    if in_body {
                        emitter
                            .emit_paragraph_break()
                            .map_err(ConvertError::Io)?;
                    }
                } else if local == b"h" {
                    if in_body {
                        if let Some((text, level)) = heading.take() {
                            emitter
                                .emit_heading(level, &text)
                                .map_err(ConvertError::Io)?;
                        }
                    } else {
                        heading = None;
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
            Event::CData(t) => {
                if in_body && skip_depth == 0 {
                    let text = match t.decode() {
                        Ok(cow) => cow.into_owned(),
                        Err(_) => String::from_utf8_lossy(t.as_ref()).into_owned(),
                    };
                    if let Some((buf, _)) = heading.as_mut() {
                        buf.push_str(&text);
                    } else {
                        emitter.emit_text(&text).map_err(ConvertError::Io)?;
                    }
                }
            }
            Event::Eof => {
                if element_depth > 0 {
                    return Err(ConvertError::Truncated);
                }
                break;
            }
            _ => {}
        }
    }
    Ok(())
}

fn heading_level(
    e: &quick_xml::events::BytesStart,
) -> Result<u8, ConvertError> {
    Ok(if attr_value(e, b"outline-level")?.as_deref() == Some("1") {
        1
    } else {
        2
    })
}

fn space_count(e: &quick_xml::events::BytesStart) -> Result<usize, ConvertError> {
    Ok(attr_value(e, b"c")?
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(1))
}

fn attr_value(
    e: &quick_xml::events::BytesStart,
    key: &[u8],
) -> Result<Option<String>, ConvertError> {
    let mut found = None;
    for attr in e.attributes() {
        let attr = attr.map_err(|e| ConvertError::Malformed(e.to_string()))?;
        if found.is_none() && attr.key.local_name().as_ref() == key {
            found = attr.unescape_value().ok().map(|v| v.into_owned());
        }
    }
    Ok(found)
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
                encoding: crate::encoding::Charset::Utf8,
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
    fn truncated_xml_is_error() {
        // Input ends mid-element; quick-xml reaches EOF silently, so the
        // converter's depth check must catch it.
        let xml = "<office:document><office:body><text:p>Hello</text:p><text:p>partial";
        assert!(matches!(convert_str(xml), Err(ConvertError::Truncated)));
    }

    #[test]
    fn mismatched_end_tag_is_error() {
        let xml = doc("<text:p>Hello</text:p><text:p>partial");
        // quick-xml itself reports the mismatched end tag
        assert!(matches!(
            convert_str(&xml),
            Err(ConvertError::Xml(_))
        ));
    }

    #[test]
    fn cdata_preserved() {
        let xml = doc("<text:p>A<![CDATA[<b>B</b>]]>C</text:p>");
        assert_eq!(convert_str(&xml).unwrap(), "A<b>B</b>C\n");
    }

    #[test]
    fn prefix_independent_matching() {
        let xml = "<o:document \
                   xmlns:o=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
                   xmlns:t=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\">\
                   <o:body><t:p>Hello</t:p></o:body></o:document>";
        assert_eq!(convert_str(xml).unwrap(), "Hello\n");
    }

    #[test]
    fn structural_elements_outside_body_ignored() {
        let xml = "<office:document>\
                   <text:p>out</text:p>\
                   <office:body><text:p>in</text:p></office:body>\
                   </office:document>";
        assert_eq!(convert_str(xml).unwrap(), "in\n");
    }

    #[test]
    fn heading_whitespace_buffered() {
        let xml = doc("<text:h>A<text:s/>B<text:tab/>C<text:line-break/>D</text:h>");
        assert_eq!(convert_str(&xml).unwrap(), "A B  C\nD\n--------\n");
    }

    #[test]
    fn duplicate_attributes_is_error() {
        // quick-xml's attribute parser reports duplicates; they must
        // propagate as malformed XML instead of being swallowed.
        let xml = doc("<text:p>A<text:s text:c=\"2\" text:c=\"9\"/>B</text:p>");
        assert!(matches!(
            convert_str(&xml),
            Err(ConvertError::Malformed(_))
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
