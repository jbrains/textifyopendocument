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

/// A target charset for output. `Ascii` exists because encoding_rs has no
/// pure ASCII encoder (the WHATWG `ascii` label maps to windows-1252).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Charset {
    Utf8,
    Ascii,
    Other(&'static encoding_rs::Encoding),
}

impl Charset {
    pub fn can_encode(&self, c: char) -> bool {
        match self {
            Charset::Utf8 => true,
            Charset::Ascii => c.is_ascii(),
            Charset::Other(enc) => {
                let mut buf = [0u8; 4];
                let (_, _, had_errors) = enc.encode(c.encode_utf8(&mut buf));
                !had_errors
            }
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Charset::Utf8 => "UTF-8",
            Charset::Ascii => "US-ASCII",
            Charset::Other(enc) => enc.name(),
        }
    }
}

const ASCII_LABELS: &[&str] = &[
    "ascii",
    "us-ascii",
    "us",
    "ansi-x3.4-1968",
    "ansi-x3.4-1986",
    "iso646-us",
    "iso-646-irv-1991",
    "cp367",
    "csascii",
    "646",
];

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

/// Detect the terminal charset from LC_ALL, LC_CTYPE, then LANG,
/// defaulting to UTF-8. The `C` and `POSIX` locales mean ASCII.
pub fn detect_codeset() -> Charset {
    for var in ["LC_ALL", "LC_CTYPE", "LANG"] {
        if let Ok(locale) = std::env::var(var) {
            if locale == "C" || locale == "POSIX" {
                return Charset::Ascii;
            }
            if let Some(codeset) = codeset_from_locale(&locale) {
                if let Ok(cs) = resolve_encoding(codeset) {
                    return cs;
                }
            }
        }
    }
    Charset::Utf8
}

pub fn resolve_encoding(label: &str) -> Result<Charset, EncodingError> {
    let normalized = label.to_ascii_lowercase().replace('_', "-");
    if ASCII_LABELS.contains(&normalized.as_str()) {
        return Ok(Charset::Ascii);
    }
    if normalized == "utf-8" || normalized == "utf8" {
        return Ok(Charset::Utf8);
    }
    encoding_rs::Encoding::for_label(normalized.as_bytes())
        .map(Charset::Other)
        .ok_or_else(|| EncodingError::Unknown(label.to_string()))
}

/// Convert UTF-8 bytes to the target charset. Characters the charset
/// cannot represent become `?`.
pub fn transcode(cs: Charset, utf8: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(utf8.len());
    let mut buf = [0u8; 4];
    for c in String::from_utf8_lossy(utf8).chars() {
        let encoded = c.encode_utf8(&mut buf);
        match cs {
            Charset::Utf8 => out.extend_from_slice(encoded.as_bytes()),
            Charset::Ascii => {
                if c.is_ascii() {
                    out.extend_from_slice(encoded.as_bytes());
                } else {
                    out.push(b'?');
                }
            }
            Charset::Other(enc) => {
                let (bytes, _, had_errors) = enc.encode(encoded);
                if had_errors {
                    out.push(b'?');
                } else {
                    out.extend_from_slice(&bytes);
                }
            }
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
        assert_eq!(resolve_encoding("UTF-8"), Ok(Charset::Utf8));
        assert_eq!(resolve_encoding("utf8"), Ok(Charset::Utf8));
        assert_eq!(
            resolve_encoding("iso-8859-1"),
            Ok(Charset::Other(encoding_rs::WINDOWS_1252))
        );
        assert!(matches!(
            resolve_encoding("not-a-codeset"),
            Err(EncodingError::Unknown(_))
        ));
    }

    #[test]
    fn resolve_ascii_labels() {
        for label in [
            "ascii",
            "us-ascii",
            "US-ASCII",
            "ANSI_X3.4-1968",
            "ISO646-US",
        ] {
            assert_eq!(
                resolve_encoding(label),
                Ok(Charset::Ascii),
                "label {:?}",
                label
            );
        }
    }

    #[test]
    fn charset_can_encode() {
        assert!(Charset::Utf8.can_encode('€'));
        assert!(Charset::Ascii.can_encode('a'));
        assert!(!Charset::Ascii.can_encode('€'));
        assert!(Charset::Other(encoding_rs::KOI8_R).can_encode('a'));
        assert!(!Charset::Other(encoding_rs::KOI8_R).can_encode('€'));
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

    #[test]
    fn transcode_utf8_is_identity() {
        assert_eq!(transcode(Charset::Utf8, "€".as_bytes()), "€".as_bytes());
    }
}
