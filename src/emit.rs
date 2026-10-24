/*
 * textifyopendocument: a fast converter from OpenDocument to plain text.
 *
 * Copyright (c) 2026 jbrains
 *
 * This program is free software; you can redistribute it and/or
 * modify it under the terms of the GNU General Public License,
 * version 2 as published by the Free Software Foundation
 */

use std::io::{self, Write};
use std::time::Instant;

use crate::args::SubstMode;
use crate::encoding::Charset;
use crate::subst::subst_for;

/// Upper bound on spaces emitted for one `text:c`/`text:tab` request.
/// Prevents absurd allocations from hostile counts; beyond this the
/// repetition is truncated (sensible for a preview tool).
pub const MAX_SPACES: usize = 4096;

pub struct EmitConfig {
    pub width: Option<usize>,       // None = no wrapping
    pub limit_chars: Option<usize>, // None = unlimited
    pub deadline: Option<Instant>,  // None = unlimited
    pub subst: SubstMode,
    pub encoding: Charset, // for SubstMode::Some checks
}

pub struct Emitter<W: Write> {
    out: W,
    cfg: EmitConfig,
    line_len: usize,
    pending_newlines: usize,
    space_pending: usize,
    wrote_content: bool,
    chars_written: usize,
    limit_reached: bool,
}

impl<W: Write> Emitter<W> {
    pub fn new(w: W, cfg: EmitConfig) -> Self {
        Emitter {
            out: w,
            cfg,
            line_len: 0,
            pending_newlines: 0,
            space_pending: 0,
            wrote_content: false,
            chars_written: 0,
            limit_reached: false,
        }
    }

    pub fn emit_text(&mut self, text: &str) -> io::Result<()> {
        if self.deadline_passed() {
            self.limit_reached = true;
            return Ok(());
        }
        let mut word = String::new();
        let mut since_deadline_check = 0usize;
        for c in text.chars() {
            if self.limit_reached {
                break;
            }
            since_deadline_check += 1;
            if since_deadline_check >= 4096 {
                since_deadline_check = 0;
                if self.deadline_passed() {
                    self.limit_reached = true;
                    break;
                }
            }
            for rc in self.replace_char(c).chars() {
                match rc {
                    '\n' => {
                        self.flush_word(&mut word)?;
                        self.request_newlines(1)?;
                    }
                    ' ' => {
                        self.flush_word(&mut word)?;
                        if !self.limit_reached() {
                            self.space_pending += 1;
                        }
                    }
                    _ => word.push(rc),
                }
            }
        }
        self.flush_word(&mut word)
    }

    pub fn emit_paragraph_break(&mut self) -> io::Result<()> {
        if self.deadline_passed() {
            self.limit_reached = true;
            return Ok(());
        }
        self.request_newlines(2)
    }

    pub fn emit_line_break(&mut self) -> io::Result<()> {
        if self.deadline_passed() {
            self.limit_reached = true;
            return Ok(());
        }
        self.request_newlines(1)
    }

    pub fn emit_spaces(&mut self, n: usize) -> io::Result<()> {
        if self.deadline_passed() {
            self.limit_reached = true;
            return Ok(());
        }
        let n = n.min(MAX_SPACES);
        if self.pending_newlines > 0 {
            self.flush_pending_newlines()?;
        }
        if self.line_len == 0 || !self.wrote_content {
            return Ok(()); // start-of-line spaces are dropped
        }
        for _ in 0..n {
            if self.limit_reached {
                break;
            }
            self.space_pending += 1;
        }
        Ok(())
    }

    pub fn emit_heading(&mut self, level: u8, text: &str) -> io::Result<()> {
        if self.deadline_passed() {
            self.limit_reached = true;
            return Ok(());
        }
        let underline = if level == 1 { '=' } else { '-' };
        self.emit_text(text)?;
        self.emit_line_break()?;
        self.emit_text(&underline.to_string().repeat(text.chars().count()))?;
        self.emit_paragraph_break()
    }

    pub fn emit_image_marker(&mut self, name: &str) -> io::Result<()> {
        if self.deadline_passed() {
            self.limit_reached = true;
            return Ok(());
        }
        let marker = format!("[-- Image: {} --]", name);
        for c in marker.chars() {
            self.emit_char(c)?;
        }
        self.request_newlines(1)
    }

    pub fn finish(mut self) -> io::Result<W> {
        if self.limit_reached {
            // a truncated preview ends where the limit fired
        } else if self.pending_newlines > 0 {
            self.write_newline()?;
        }
        self.out.flush()?;
        Ok(self.out)
    }

    pub fn limit_reached(&self) -> bool {
        self.limit_reached
    }

    pub fn chars_written(&self) -> usize {
        self.chars_written
    }

    /// Re-check the deadline; sets limit_reached when it has passed.
    /// Callers that consume many events without emitting (e.g. the
    /// conversion loop) call this periodically so time limits hold.
    pub fn check_limits(&mut self) {
        if self.deadline_passed() {
            self.limit_reached = true;
        }
    }

    fn deadline_passed(&self) -> bool {
        self.cfg.deadline.map_or(false, |d| Instant::now() >= d)
    }

    fn replace_char(&self, c: char) -> std::borrow::Cow<'static, str> {
        use std::borrow::Cow;
        match self.cfg.subst {
            SubstMode::None => Cow::Owned(c.to_string()),
            SubstMode::All => match subst_for(c) {
                Some(s) => Cow::Borrowed(s),
                None => Cow::Owned(c.to_string()),
            },
            SubstMode::Some => {
                if self.cfg.encoding.can_encode(c) {
                    Cow::Owned(c.to_string())
                } else {
                    match subst_for(c) {
                        Some(s) => Cow::Borrowed(s),
                        None => Cow::Borrowed("?"),
                    }
                }
            }
        }
    }

    fn flush_word(&mut self, word: &mut String) -> io::Result<()> {
        if word.is_empty() {
            return Ok(());
        }
        if self.pending_newlines > 0 {
            self.flush_pending_newlines()?;
        }
        if self.space_pending > 0 {
            let spaces = self.space_pending;
            self.space_pending = 0;
            if self.line_len > 0 {
                if self.need_break(spaces, word.chars().count()) {
                    self.request_newlines(1)?;
                } else {
                    for _ in 0..spaces {
                        self.emit_char(' ')?;
                    }
                }
            }
            // line_len == 0: spaces at start of line are dropped
        }
        for c in word.chars() {
            self.emit_char(c)?;
        }
        word.clear();
        Ok(())
    }

    fn need_break(&self, spaces: usize, word_len: usize) -> bool {
        self.cfg
            .width
            .map_or(false, |w| self.line_len + spaces + word_len > w)
    }

    fn request_newlines(&mut self, n: usize) -> io::Result<()> {
        self.space_pending = 0; // trailing spaces before a newline are dropped
        if !self.wrote_content {
            return Ok(()); // leading blank lines are dropped
        }
        self.pending_newlines = (self.pending_newlines + n).min(2);
        Ok(())
    }

    fn flush_pending_newlines(&mut self) -> io::Result<()> {
        while self.pending_newlines > 0 && !self.limit_reached {
            self.write_newline()?;
            self.pending_newlines -= 1;
        }
        self.pending_newlines = 0;
        Ok(())
    }

    fn write_newline(&mut self) -> io::Result<()> {
        if self.chars_written >= self.cfg.limit_chars.unwrap_or(usize::MAX) {
            self.limit_reached = true;
            return Ok(());
        }
        self.out.write_all(b"\n")?;
        self.chars_written += 1;
        self.line_len = 0;
        if self.chars_written >= self.cfg.limit_chars.unwrap_or(usize::MAX) {
            self.limit_reached = true;
        }
        Ok(())
    }

    fn emit_char(&mut self, c: char) -> io::Result<()> {
        if self.limit_reached {
            return Ok(());
        }
        if self.pending_newlines > 0 {
            self.flush_pending_newlines()?;
        }
        if self.chars_written >= self.cfg.limit_chars.unwrap_or(usize::MAX) {
            self.limit_reached = true;
            return Ok(());
        }
        let mut buf = [0u8; 4];
        self.out.write_all(c.encode_utf8(&mut buf).as_bytes())?;
        self.chars_written += 1;
        self.line_len += 1;
        self.wrote_content = true;
        if self.chars_written >= self.cfg.limit_chars.unwrap_or(usize::MAX) {
            self.limit_reached = true;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::SubstMode;
    use std::time::Instant;

    fn cfg() -> EmitConfig {
        EmitConfig {
            width: None,
            limit_chars: None,
            deadline: None,
            subst: SubstMode::None,
            encoding: crate::encoding::Charset::Utf8,
        }
    }

    fn emit_with(cfg: EmitConfig, f: impl FnOnce(&mut Emitter<Vec<u8>>)) -> String {
        let mut e = Emitter::new(Vec::new(), cfg);
        f(&mut e);
        String::from_utf8(e.finish().unwrap()).unwrap()
    }

    #[test]
    fn wrap_at_spaces() {
        let mut c = cfg();
        c.width = Some(10);
        let out = emit_with(c, |e| {
            e.emit_text("one two three").unwrap();
        });
        assert_eq!(out, "one two\nthree");
    }

    #[test]
    fn no_wrap_when_width_none() {
        let out = emit_with(cfg(), |e| {
            e.emit_text("one two three").unwrap();
        });
        assert_eq!(out, "one two three");
    }

    #[test]
    fn newlines_cap_at_two() {
        let out = emit_with(cfg(), |e| {
            e.emit_text("x").unwrap();
            e.emit_paragraph_break().unwrap();
            e.emit_paragraph_break().unwrap();
            e.emit_paragraph_break().unwrap();
            e.emit_text("y").unwrap();
        });
        assert_eq!(out, "x\n\ny");
    }

    #[test]
    fn start_of_line_spaces_dropped() {
        let out = emit_with(cfg(), |e| {
            e.emit_text("x").unwrap();
            e.emit_paragraph_break().unwrap();
            e.emit_text("  indented").unwrap();
        });
        assert_eq!(out, "x\n\nindented");
    }

    #[test]
    fn start_of_line_emit_spaces_dropped() {
        let out = emit_with(cfg(), |e| {
            e.emit_text("x").unwrap();
            e.emit_paragraph_break().unwrap();
            e.emit_spaces(4).unwrap();
            e.emit_text("y").unwrap();
        });
        assert_eq!(out, "x\n\ny");
    }

    #[test]
    fn leading_blank_lines_dropped() {
        let out = emit_with(cfg(), |e| {
            e.emit_paragraph_break().unwrap();
            e.emit_paragraph_break().unwrap();
            e.emit_text("x").unwrap();
        });
        assert_eq!(out, "x");
    }

    #[test]
    fn trailing_spaces_before_newline_dropped() {
        let out = emit_with(cfg(), |e| {
            e.emit_text("foo ").unwrap();
            e.emit_paragraph_break().unwrap();
        });
        assert_eq!(out, "foo\n");
    }

    #[test]
    fn finish_trailing_newlines() {
        let out = emit_with(cfg(), |e| {
            e.emit_text("x").unwrap();
            e.emit_paragraph_break().unwrap();
            e.emit_paragraph_break().unwrap();
            e.emit_paragraph_break().unwrap();
        });
        assert_eq!(out, "x\n");

        let out = emit_with(cfg(), |e| {
            e.emit_text("x").unwrap();
            e.emit_paragraph_break().unwrap();
        });
        assert_eq!(out, "x\n");

        let out = emit_with(cfg(), |e| {
            e.emit_text("x").unwrap();
        });
        assert_eq!(out, "x");
    }

    #[test]
    fn heading_underlines() {
        let out = emit_with(cfg(), |e| {
            e.emit_heading(1, "Title").unwrap();
        });
        assert_eq!(out, "Title\n=====\n");

        let out = emit_with(cfg(), |e| {
            e.emit_heading(2, "Sub").unwrap();
        });
        assert_eq!(out, "Sub\n---\n");
    }

    #[test]
    fn headings_separated_by_blank_line() {
        let out = emit_with(cfg(), |e| {
            e.emit_heading(1, "Alpha").unwrap();
            e.emit_heading(2, "Beta").unwrap();
        });
        assert_eq!(out, "Alpha\n=====\n\nBeta\n----\n");
    }

    #[test]
    fn image_marker() {
        let out = emit_with(cfg(), |e| {
            e.emit_image_marker("pic1").unwrap();
        });
        assert_eq!(out, "[-- Image: pic1 --]\n");
    }

    #[test]
    fn char_limit_truncates() {
        let mut c = cfg();
        c.limit_chars = Some(5);
        let mut e = Emitter::new(Vec::new(), c);
        e.emit_text("abcdef").unwrap();
        assert!(e.limit_reached());
        assert_eq!(e.chars_written(), 5);
        e.emit_text("xyz").unwrap();
        let out = String::from_utf8(e.finish().unwrap()).unwrap();
        assert_eq!(out, "abcde");
    }

    #[test]
    fn deadline_stops() {
        let mut c = cfg();
        c.deadline = Some(Instant::now() - std::time::Duration::from_secs(1));
        let mut e = Emitter::new(Vec::new(), c);
        e.emit_text("hello").unwrap();
        assert!(e.limit_reached());
        assert_eq!(e.chars_written(), 0);
        let out = String::from_utf8(e.finish().unwrap()).unwrap();
        assert_eq!(out, "");
    }

    #[test]
    fn subst_some_all_none() {
        // SubstMode::Some replaces only what the target encoding cannot
        // represent: '€' and 'ä' via the table, '☃' via '?'.
        let mut c = cfg();
        c.subst = SubstMode::Some;
        c.encoding = crate::encoding::Charset::Other(encoding_rs::KOI8_R); // lacks €, ä, ☃
        let out = emit_with(c, |e| {
            e.emit_text("€ä☃").unwrap();
        });
        assert_eq!(out, "EURae?");

        // SubstMode::All replaces even when the encoding could represent it.
        let mut c = cfg();
        c.subst = SubstMode::All;
        let out = emit_with(c, |e| {
            e.emit_text("€").unwrap();
        });
        assert_eq!(out, "EUR");

        // SubstMode::None keeps everything.
        let out = emit_with(cfg(), |e| {
            e.emit_text("€").unwrap();
        });
        assert_eq!(out, "€");
    }

    #[test]
    fn multibyte_wrap_and_limit() {
        let mut c = cfg();
        c.width = Some(4);
        c.limit_chars = Some(7);
        let out = emit_with(c, |e| {
            e.emit_text("😀😀😀😀").unwrap();
        });
        assert_eq!(out, "😀😀😀😀"); // a word of exactly 4 chars is not split

        let mut c = cfg();
        c.width = Some(4);
        let out = emit_with(c, |e| {
            e.emit_text("a😀 b").unwrap();
        });
        assert_eq!(out, "a😀 b"); // emoji counts as one column
    }

    #[test]
    fn multibyte_char_limit_truncates_mid_word() {
        let mut c = cfg();
        c.limit_chars = Some(3);
        let mut e = Emitter::new(Vec::new(), c);
        e.emit_text("😀😀😀😀").unwrap();
        assert!(e.limit_reached());
        let out = String::from_utf8(e.finish().unwrap()).unwrap();
        assert_eq!(out, "😀😀😀");
    }

    #[test]
    fn char_limit_set_immediately_at_exact_count() {
        let mut c = cfg();
        c.limit_chars = Some(5);
        let mut e = Emitter::new(Vec::new(), c);
        e.emit_text("abcde").unwrap();
        assert!(e.limit_reached());
        assert_eq!(e.chars_written(), 5);
    }

    #[test]
    fn emit_spaces_huge_count_does_not_panic() {
        let mut e = Emitter::new(Vec::new(), cfg());
        e.emit_text("A").unwrap();
        e.emit_spaces(usize::MAX).unwrap();
        e.emit_text("B").unwrap();
        let out = String::from_utf8(e.finish().unwrap()).unwrap();
        let mut expected = String::from("A");
        expected.push_str(&" ".repeat(4096));
        expected.push('B');
        assert_eq!(out, expected);
    }

    #[test]
    fn subst_ascii_semantics() {
        let mut c = cfg();
        c.subst = SubstMode::Some;
        c.encoding = crate::encoding::Charset::Ascii;
        let out = emit_with(c, |e| {
            e.emit_text("€ä☃").unwrap();
        });
        assert_eq!(out, "EURae?");
    }
}
