/*
 * textifyopendocument: a fast converter from OpenDocument to plain text.
 *
 * Copyright (c) 2026 jbrains
 *
 * This program is free software; you can redistribute it and/or
 * modify it under the terms of the GNU General Public License,
 * version 2 as published by the Free Software Foundation
 */

// Values are the *decoded* text: the C tool stored `&lt;` etc. and decoded
// them in a later pass; we emit the final text directly.
pub fn subst_for(c: char) -> Option<&'static str> {
    match c {
        '\u{00A0}' => Some(" "), // no-break space
        '©' => Some("(c)"),
        '«' => Some("<<"),
        '\u{00AD}' => Some("-"), // soft hyphen
        '®' => Some("(r)"),
        '»' => Some(">>"),
        '¼' => Some("1/4"),
        '½' => Some("1/2"),
        '¾' => Some("3/4"),
        'Ä' => Some("Ae"),
        'Ö' => Some("Oe"),
        'Ü' => Some("Ue"),
        'ß' => Some("ss"),
        'ä' => Some("ae"),
        'ö' => Some("oe"),
        'ü' => Some("ue"),
        '\u{2010}' => Some("-"), // hyphen
        '\u{2011}' => Some("-"), // non-breaking hyphen
        '\u{2012}' => Some("-"), // figure dash
        '–' => Some("-"),        // en dash
        '—' => Some("--"),       // em dash
        '\u{2015}' => Some("--"), // quotation dash
        '\u{2018}' => Some("`"),
        '\u{2019}' => Some("'"),
        '\u{201A}' => Some(","),
        '\u{201B}' => Some("`"),
        '\u{201C}' => Some("``"),
        '\u{201D}' => Some("''"),
        '\u{201E}' => Some(",,"),
        '•' => Some("o "),      // bullet
        '‣' => Some("< "),      // U+2023, the C table's mislabeled entry
        '\u{2025}' => Some(".."),
        '…' => Some("..."),
        '‰' => Some("o/oo"),
        '‹' => Some("<"),
        '›' => Some(">"),
        '€' => Some("EUR"),
        '←' => Some("<-"),
        '→' => Some("->"),
        '↔' => Some("<->"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_look_alikes() {
        assert_eq!(subst_for('€'), Some("EUR"));
        assert_eq!(subst_for('©'), Some("(c)"));
        assert_eq!(subst_for('«'), Some("<<"));
        assert_eq!(subst_for('ä'), Some("ae"));
        assert_eq!(subst_for('—'), Some("--"));
        assert_eq!(subst_for('•'), Some("o "));
        assert_eq!(subst_for('‣'), Some("< ")); // U+2023, the C table's mislabeled entry
    }

    #[test]
    fn unknown_chars() {
        assert_eq!(subst_for('a'), None);
        assert_eq!(subst_for('☃'), None);
    }

    #[test]
    fn every_table_row() {
        // The full table from the spec Appendix (40 rows), no exceptions.
        const TABLE: &[(char, &str)] = &[
            ('\u{00A0}', " "),
            ('©', "(c)"),
            ('«', "<<"),
            ('\u{00AD}', "-"),
            ('®', "(r)"),
            ('»', ">>"),
            ('¼', "1/4"),
            ('½', "1/2"),
            ('¾', "3/4"),
            ('Ä', "Ae"),
            ('Ö', "Oe"),
            ('Ü', "Ue"),
            ('ß', "ss"),
            ('ä', "ae"),
            ('ö', "oe"),
            ('ü', "ue"),
            ('\u{2010}', "-"),
            ('\u{2011}', "-"),
            ('\u{2012}', "-"),
            ('–', "-"),
            ('—', "--"),
            ('\u{2015}', "--"),
            ('\u{2018}', "`"),
            ('\u{2019}', "'"),
            ('\u{201A}', ","),
            ('\u{201B}', "`"),
            ('\u{201C}', "``"),
            ('\u{201D}', "''"),
            ('\u{201E}', ",,"),
            ('•', "o "),
            ('‣', "< "),
            ('\u{2025}', ".."),
            ('…', "..."),
            ('‰', "o/oo"),
            ('‹', "<"),
            ('›', ">"),
            ('€', "EUR"),
            ('←', "<-"),
            ('→', "->"),
            ('↔', "<->"),
        ];
        for &(c, expected) in TABLE {
            assert_eq!(
                subst_for(c),
                Some(expected),
                "substitution for U+{:04X}",
                c as u32
            );
        }
    }
}
