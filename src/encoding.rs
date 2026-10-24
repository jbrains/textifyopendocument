/*
 * textifyopendocument: a fast converter from OpenDocument to plain text.
 *
 * Copyright (c) 2026 jbrains
 *
 * This program is free software; you can redistribute it and/or
 * modify it under the terms of the GNU General Public License,
 * version 2 as published by the Free Software Foundation
 */

#[derive(Debug, PartialEq)]
pub enum EncodingError {
    Unknown(String),
}

/// Pure helper: extract the codeset from a locale string such as
/// `en_US.UTF-8` or `de_DE.ISO-8859-15@euro`. Returns `None` when the
/// locale carries no codeset (e.g. `C`).
pub fn codeset_from_locale(locale: &str) -> Option<&str> {
    let after_dot = locale.rsplit('.').next()?;
    if after_dot == locale {
        return None; // no '.' in the locale at all
    }
    let codeset = after_dot.split('@').next().unwrap_or(after_dot);
    if codeset.is_empty() {
        None
    } else {
        Some(codeset)
    }
}

/// Detect the terminal codeset from LC_ALL, LC_CTYPE, then LANG,
/// defaulting to UTF-8.
pub fn detect_codeset() -> &'static encoding_rs::Encoding {
    for var in ["LC_ALL", "LC_CTYPE", "LANG"] {
        if let Ok(locale) = std::env::var(var) {
            if let Some(codeset) = codeset_from_locale(&locale) {
                if let Ok(enc) = resolve_encoding(codeset) {
                    return enc;
                }
            }
        }
    }
    encoding_rs::UTF_8
}

pub fn resolve_encoding(
    label: &str,
) -> Result<&'static encoding_rs::Encoding, EncodingError> {
    encoding_rs::Encoding::for_label(label.as_bytes())
        .ok_or_else(|| EncodingError::Unknown(label.to_string()))
}

/// Convert UTF-8 bytes to the target encoding. Characters the encoding
/// cannot represent become `?`.
pub fn transcode(enc: &'static encoding_rs::Encoding, utf8: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(utf8.len());
    let mut buf = [0u8; 4];
    for c in String::from_utf8_lossy(utf8).chars() {
        let (bytes, _, had_errors) = enc.encode(c.encode_utf8(&mut buf));
        if had_errors {
            out.push(b'?');
        } else {
            out.extend_from_slice(&bytes);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codeset_parsing() {
        assert_eq!(codeset_from_locale("en_US.UTF-8"), Some("UTF-8"));
        assert_eq!(codeset_from_locale("C"), None);
        assert_eq!(
            codeset_from_locale("de_DE.ISO-8859-15@euro"),
            Some("ISO-8859-15")
        );
    }

    #[test]
    fn resolve() {
        assert_eq!(resolve_encoding("UTF-8"), Ok(encoding_rs::UTF_8));
        assert_eq!(resolve_encoding("iso-8859-1"), Ok(encoding_rs::WINDOWS_1252));
        assert!(matches!(
            resolve_encoding("not-a-codeset"),
            Err(EncodingError::Unknown(_))
        ));
    }

    #[test]
    fn transcode_ascii() {
        let ascii = resolve_encoding("ascii").unwrap();
        assert_eq!(transcode(ascii, "Uber".as_bytes()), b"Uber");
    }

    #[test]
    fn transcode_unencodable_becomes_question_mark() {
        let ascii = resolve_encoding("ascii").unwrap();
        assert_eq!(transcode(ascii, "☃".as_bytes()), b"?");
    }
}
