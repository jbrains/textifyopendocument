# textifyopendocument Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `textifyopendocument`, a fast Rust CLI that converts OpenDocument files to plain text for previews, with full-text and truncated-preview modes.

**Architecture:** One streaming pipeline: open the input (zip or raw XML), feed `content.xml` through a `quick-xml` pull parser into a state machine that emits shaped text (wrapping, newline capping, substitutions, limits), transcode to the terminal encoding, write out. No document tree is built; truncation stops the parse.

**Tech Stack:** Rust (edition 2021), crates: `quick-xml` 0.37, `zip` 2 (deflate only), `encoding_rs` 0.8. No CLI framework.

**Spec:** `docs/superpowers/specs/2026-10-24-textifyopendocument-design.md`

## Global Constraints

- License: GPL-2. Every new source file starts with the GPL-2 header boilerplate (`/* … This program is free software; you can redistribute it and/or modify it under the terms of the GNU General Public License, version 2 … */` or the rustdoc equivalent), and attribution stays intact (AUTHORS, README, man page).
- Platform: Linux only. Never hard-code glibc specifics beyond the locale env vars `LC_ALL`, `LC_CTYPE`, `LANG`.
- Not a drop-in replacement: sensible plain text; no byte-compatibility with the C `odt2txt` is required.
- Package/binary name: `textifyopendocument`.
- CLI: `--width=N` (default 65; `-1` disables wrapping; any other value < 3 is invalid), `--output=FILE`, `--raw`, `--raw-input`, `--encoding=X` and `--encoding=show`, `--subst=none|some|all` (default `some`), `--force` (accepted and ignored), `--version`, `--help`. New: `--limit-chars=N` (Unicode scalar values, newlines included), `--limit-time=N` (milliseconds).
- Two modes: full text is the default; a char or time limit enables truncated preview, whichever limit fires first wins.
- Streaming single pass over the XML; the C tool's regex list is the checklist: each row of the spec's mapping table gets a test (incremental strategy).
- Exit codes: 0 on success, 1 on any error.
- Dependencies: only the three crates above (plus std). Test fixtures are generated in code; no dev-dependencies.

## Review Focus

The five input classes most likely to bite a user, not covered by task tests until pinned here:

1. **Input that isn't a zip, or a zip without `content.xml`** — must print a clear error naming the file, exit 1, never panic. → pinned in Task 5.
2. **Malformed or truncated XML** — must not panic or hang; convert returns an error (exit 1), and text already emitted stays emitted. → pinned in Task 6.
3. **Empty document / no `office:body`** — emits nothing, exits 0. → pinned in Task 6.
4. **Multibyte content (emoji, CJK) with wrapping and `--limit-chars`** — wrapping counts characters not bytes, limits count characters, no mid-codepoint slicing or panics. → pinned in Task 4.
5. **Target encoding cannot represent characters** (e.g. `LANG=C` or `--encoding=ascii`) — with `--subst=some` look-alikes are substituted; anything still unencodable becomes `?`; never panics or loops. → pinned in Tasks 4 and 8.

---

## File Structure

- `Cargo.toml` — package metadata, the three dependencies.
- `.gitignore` — `/target`.
- `src/main.rs` — entry point: parse args, dispatch `Action`, map `Error` to stderr + exit code.
- `src/args.rs` — `Config`, `SubstMode`, `Action`, `parse_args`.
- `src/subst.rs` — the look-alike table and `subst_for`.
- `src/emit.rs` — `Emitter`: all output shaping (wrap, newline capping, start-of-line, subst, limits, headings, markers).
- `src/source.rs` — open `content.xml` from a zip or a raw file.
- `src/convert.rs` — `quick-xml` event loop → `Emitter` calls (the spec's mapping table).
- `src/encoding.rs` — codeset detection, label resolution, UTF-8 → target transcoding with `?` fallback.
- `src/lib.rs` — `run(Config)` wiring the pipeline; `Error` enum.
- `tests/support/mod.rs` — in-code fixture builders (zip writing, temp files).
- `tests/cli.rs` — end-to-end tests driving the compiled binary.
- `tests/golden.rs` — full-document golden tests.
- `tests/perf.rs` — `#[ignore]` performance/limit tests on a generated 20k-row spreadsheet.

---

### Task 1: Scaffold the crate

**Files:**
- Create: `Cargo.toml`, `.gitignore`, `src/main.rs`, `src/lib.rs`, `tests/cli.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: a building crate named `textifyopendocument`; binary that answers `--version` and `--help`.

- [ ] **Step 1: Write `Cargo.toml`**

```toml
[package]
name = "textifyopendocument"
version = "0.1.0"
edition = "2021"
license = "GPL-2.0-only"
description = "Fast OpenDocument to plain text converter for previews"

[dependencies]
quick-xml = "0.37"
zip = { version = "2", default-features = false, features = ["deflate"] }
encoding_rs = "0.8"
```

Add `/target` to `.gitignore`. Start `src/main.rs` and `src/lib.rs` with the GPL-2 header, `main()` calling `std::process::exit(0)`, and an empty `lib.rs`.

- [ ] **Step 2: Write the failing CLI smoke test in `tests/cli.rs`**

```rust
use std::process::Command;

fn bin() -> Command { Command::new(env!("CARGO_BIN_EXE_textifyopendocument")) }

#[test]
fn version_flag_prints_name_and_exits_zero() {
    let out = bin().arg("--version").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("textifyopendocument"));
}
```

- [ ] **Step 3: Run it to verify it fails**

Run: `cargo test`
Expected: FAIL — `--version` prints nothing (exit 0 but no name).

- [ ] **Step 4: Implement `--version` and `--help` in `src/main.rs`**

Literal parsing for now (replaced in Task 2): if the first arg is `--version`, print `textifyopendocument 0.1.0` and exit 0; if `--help`, print a one-line usage placeholder and exit 0.

- [ ] **Step 5: Run to verify it passes**

Run: `cargo test`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml .gitignore src/main.rs src/lib.rs tests/cli.rs
git commit -m "feat: scaffold textifyopendocument crate"
```

---

### Task 2: Argument parsing

**Files:**
- Create: `src/args.rs`
- Modify: `src/main.rs`
- Test: `src/args.rs` (`#[cfg(test)]`), `tests/cli.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:

```rust
// src/args.rs
#[derive(Debug, Clone, PartialEq)]
pub enum SubstMode { None, Some, All }

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub input: PathBuf,
    pub raw: bool,
    pub raw_input: bool,
    pub width: i64,                // default 65; -1 = no wrap; other values < 3 invalid
    pub output: Option<PathBuf>,
    pub encoding: Option<String>,
    pub subst: SubstMode,          // default SubstMode::Some
    pub limit_chars: Option<usize>,
    pub limit_time_ms: Option<u64>,
}

#[derive(Debug, PartialEq)]
pub enum Action { Run(Config), ShowEncoding, Help, Version }

#[derive(Debug, PartialEq)]
pub enum CliError {
    MissingFilename,
    UnknownOption(String),
    InvalidWidth(String),
    InvalidSubst(String),
    InvalidLimit(String),
}

pub fn parse_args(args: &[String]) -> Result<Action, CliError>;
```

`args` is the argument vector *after* the program name. `--force` is accepted and ignored. `--encoding=show` returns `Action::ShowEncoding`. Exactly one positional filename is required; a second positional is `CliError::UnknownOption`.

- [ ] **Step 1: Write the failing tests in `src/args.rs`**

`parse_defaults`: `["doc.odt"]` → `Run(Config { width: 65, subst: SubstMode::Some, raw: false, raw_input: false, output: None, encoding: None, limit_chars: None, limit_time_ms: None, input: "doc.odt".into() })`.
`parse_each_flag`: one test per flag — `--width=80` → width 80; `--width=-1` → -1; `--raw`, `--raw-input`, `--output=/tmp/out.txt`, `--encoding=UTF-8`, `--subst=none`, `--subst=all`, `--limit-chars=5000` → `limit_chars: Some(5000)`, `--limit-time=200` → `limit_time_ms: Some(200)`; `--force` accepted and ignored.
`parse_errors`: `[]` → `CliError::MissingFilename`; `["--width=2"]` → `InvalidWidth`; `["--subst=sometimes"]` → `InvalidSubst`; `["--limit-chars=abc"]` → `InvalidLimit`; `["a", "b"]` → `UnknownOption`; `["--bogus"]` → `UnknownOption`.
`parse_show_encoding`: `["--encoding=show"]` → `Action::ShowEncoding`; `["--help"]` → `Action::Help`; `["--version"]` → `Action::Version`.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test args`
Expected: FAIL — module missing.

- [ ] **Step 3: Implement `parse_args` in `src/args.rs`**

Hand-rolled loop over args, matching the exact prefixes (`--width=`, `--output=`, `--encoding=`, `--subst=`, `--limit-chars=`, `--limit-time=`). `--limit-time=N` parses N as `u64` milliseconds. `--output=-` (stdout) is treated as no `output` file, as the C tool does.

- [ ] **Step 4: Wire `main()`**

`main()` parses `std::env::args().skip(1)`, then: `Help` → print usage, exit 0; `Version` → print version, exit 0; `ShowEncoding` → placeholder that prints `UTF-8` for now (Task 8 replaces it); `Run(_config)` → print a "not yet implemented" error to stderr and exit 1.

- [ ] **Step 5: Run to verify they pass**

Run: `cargo test`
Expected: PASS (Task 1's `--version` test still passes)

- [ ] **Step 6: Commit**

```bash
git add src/args.rs src/main.rs tests/cli.rs
git commit -m "feat: parse all CLI flags into Config"
```

---

### Task 3: Look-alike substitution table

**Files:**
- Create: `src/subst.rs`
- Modify: `src/lib.rs` (declare module)
- Test: `src/subst.rs` (`#[cfg(test)]`)

**Interfaces:**
- Consumes: nothing.
- Produces: `pub fn subst_for(c: char) -> Option<&'static str>` — returns the look-alike for `c`, or `None`. Uses the exact table in the spec Appendix (36 rows, with U+2023 → `"< "`).

- [ ] **Step 1: Write the failing tests in `src/subst.rs`**

```rust
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
```

A second test iterates over a const array of all 36 `(char, &str)` pairs copied from the spec Appendix and asserts each one; include every row, no exceptions.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test subst`
Expected: FAIL — `subst_for` not found.

- [ ] **Step 3: Implement `subst_for` in `src/subst.rs`**

A `match` over `char` literals (fast, no hashing), returning the strings from the spec Appendix. Note in a comment: values are the *decoded* text (the C tool stored `&lt;` etc. and decoded them in a later pass).

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test subst`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/subst.rs src/lib.rs
git commit -m "feat: add look-alike substitution table"
```

---

### Task 4: The Emitter

**Files:**
- Create: `src/emit.rs`
- Modify: `src/lib.rs` (declare module)
- Test: `src/emit.rs` (`#[cfg(test)]`)

**Interfaces:**
- Consumes: `subst_for` (Task 3), `SubstMode` (Task 2).
- Produces:

```rust
// src/emit.rs
pub struct EmitConfig {
    pub width: Option<usize>,       // None = no wrapping
    pub limit_chars: Option<usize>, // None = unlimited
    pub deadline: Option<Instant>,  // None = unlimited
    pub subst: SubstMode,
    pub encoding: &'static encoding_rs::Encoding, // for SubstMode::Some checks
}

pub struct Emitter<W: Write> { /* … */ }

impl<W: Write> Emitter<W> {
    pub fn new(w: W, cfg: EmitConfig) -> Self;
    pub fn emit_text(&mut self, text: &str) -> io::Result<()>;
    pub fn emit_paragraph_break(&mut self) -> io::Result<()>; // blank line
    pub fn emit_line_break(&mut self) -> io::Result<()>;
    pub fn emit_spaces(&mut self, n: usize) -> io::Result<()>;
    pub fn emit_heading(&mut self, level: u8, text: &str) -> io::Result<()>; // underline '=' for 1, '-' otherwise
    pub fn emit_image_marker(&mut self, name: &str) -> io::Result<()>;      // "[-- Image: NAME --]\n"
    pub fn finish(mut self) -> io::Result<W>; // trailing-newline normalization, flush, return inner writer
    pub fn limit_reached(&self) -> bool;
    pub fn chars_written(&self) -> usize;     // Unicode scalar values, newlines included
}
```

**Behavior pinned by tests (all of it):**
- Wrapping at spaces only; a word longer than the width is never split; leading spaces after a line break are dropped (the C `\n +` rule). Default test: width 10, text `"one two three"` → `"one two\nthree"`; width `None` → unchanged.
- Consecutive newlines cap at 2 (the C `\n{3,}` rule): two `emit_paragraph_break`s in a row emit `\n\n`, never more.
- Start-of-line spaces are dropped even for `emit_spaces`.
- Nothing at all is written until the first non-newline character: leading blank lines from empty initial paragraphs are dropped (the C `^\n+` rule). `emit_paragraph_break` before any text writes nothing.
- Spaces immediately before an emitted newline are dropped (the C ` +\n` rule): `emit_text("foo ")` then `emit_paragraph_break()` emits `"foo\n\n"`, never `"foo \n\n"`.
- `finish()` normalizes trailing newlines: 2+ trailing newlines become exactly one; a single trailing newline stays; no newline stays absent.
- `emit_heading(1, "Title")` → `"Title\n=====\n\n"`; `emit_heading(2, "Sub")` → `"Sub\n---\n\n"`; underline length is the char count of the text.
- `emit_image_marker("pic1")` → `"[-- Image: pic1 --]\n"`.
- `limit_chars: Some(5)`, text `"abcdef"` → writes `"abcde"`, `limit_reached()` is `true`, further `emit_*` calls write nothing.
- `deadline` already past → the first `emit_*` call writes nothing and `limit_reached()` is `true`.
- Substitutions: with `subst: SubstMode::Some, encoding: ASCII`, `"€ä☃"` → `"EURae?"` (table hit, table hit, unencodable-and-unknown → `?`); with `subst: SubstMode::All, encoding: UTF_8`, `"€"` → `"EUR"`; with `subst: SubstMode::None, encoding: UTF_8`, `"€"` stays `"€"`. The "can this encoding encode the char" check is `encoding.can_encode(c.encode_utf8(&mut [0; 4]))`.
- **Review Focus #4:** width 4, `limit_chars: Some(7)`, text `"😀😀😀😀"` → writes exactly 4 chars (a word longer than width is not split) and stops; width 4, text `"a😀 b"` → counts the emoji as one column.

- [ ] **Step 1: Write the failing tests in `src/emit.rs`**

One test function per bullet above (`wrap_at_spaces`, `no_wrap_when_width_none`, `newlines_cap_at_two`, `start_of_line_spaces_dropped`, `leading_blank_lines_dropped`, `trailing_spaces_before_newline_dropped`, `finish_trailing_newlines`, `heading_underlines`, `image_marker`, `char_limit_truncates`, `deadline_stops`, `subst_some_all_none`, `multibyte_wrap_and_limit`).

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test emit`
Expected: FAIL — module missing.

- [ ] **Step 3: Implement `Emitter` in `src/emit.rs`**

Internal state: the wrapped writer, a `Vec<u8>`-free approach is not required — write straight through; track `line_len` (chars since last newline), `pending_newlines` (cap 2), `at_line_start`, `chars_written`, and the limits. `emit_text` iterates chars, applies the substitution table per `subst` mode, writes UTF-8 bytes. Wrapping logic: remember the last written space position of the current line; when `line_len` exceeds `width`, break at that space. `emit_heading` uses `emit_text` for the text, then `emit_line_break`, then an underline run of `text.chars().count()` chars, then `emit_paragraph_break`. Every `emit_*` entry checks `deadline` and `limit_chars` first and sets `limit_reached`.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test emit`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/emit.rs src/lib.rs
git commit -m "feat: add output-shaping emitter with wrap, limits, and substitutions"
```

---

### Task 5: Opening the source

**Files:**
- Create: `src/source.rs`, `tests/support/mod.rs`
- Modify: `src/lib.rs` (declare module)
- Test: `src/source.rs` (`#[cfg(test)]`)

**Interfaces:**
- Consumes: `Config` (Task 2).
- Produces:

```rust
// src/source.rs
pub enum SourceError {
    NotOpenDocument(String),   // file not a zip, or zip has no content.xml
    Io(io::Error),
}

pub fn open_content_xml(config: &Config) -> Result<Box<dyn BufRead>, SourceError>;
```

`open_content_xml` honors `config.raw_input` (read the file directly, `File` → `BufReader`) and otherwise opens the zip (`zip::ZipArchive`) and returns a `BufReader` over the `content.xml` entry via `archive.by_name("content.xml")`. Any failure to open/parse the zip or locate the entry is `NotOpenDocument` with the filename in the message.

`tests/support/mod.rs` provides (for this and later tasks):

```rust
// tests/support/mod.rs
pub fn minimal_zip(entries: &[(&str, Vec<u8>)]) -> Vec<u8>;   // zip::ZipWriter over Cursor
pub fn content_xml(body: &str) -> String; // full ODF wrapper with office:body
pub fn write_temp_file(name: &str, bytes: &[u8]) -> PathBuf;  // std::env::temp_dir(), unique via process id + counter
```

- [ ] **Step 1: Write the failing tests in `src/source.rs`**

Using `tests/support`, build: a zip containing `content.xml` (read back first bytes match); a zip *without* `content.xml`; and a plain-text file. Tests: `reads_content_xml_from_zip`, `raw_input_reads_file_directly`, `missing_content_xml_is_not_open_document`, `non_zip_is_not_open_document` (**Review Focus #1**). The last two assert `matches!(err, SourceError::NotOpenDocument(_))`.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test source`
Expected: FAIL — module missing.

- [ ] **Step 3: Implement `open_content_xml` and `tests/support/mod.rs`**

For the zip path: `File::open` → `ZipArchive::new` → `by_name("content.xml")` → wrap the `ZipFile` in `BufReader` and box it. Map `zip::result::ZipError::FileNotFound` and any `InvalidArchive` to `NotOpenDocument`; map the rest of zip errors and `File::open` errors to `Io` where they are IO errors.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test source`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/source.rs src/lib.rs tests/support/mod.rs
git commit -m "feat: open content.xml from zips and raw files"
```

---

### Task 6: The conversion state machine — structural elements

**Files:**
- Create: `src/convert.rs`
- Modify: `src/lib.rs` (declare module)
- Test: `src/convert.rs` (`#[cfg(test)]`)

**Interfaces:**
- Consumes: `Emitter` (Task 4).
- Produces:

```rust
// src/convert.rs
pub enum ConvertError {
    Xml(quick_xml::Error),
    Io(io::Error),
}

pub fn convert<R: BufRead, W: Write>(
    reader: &mut quick_xml::Reader<R>,
    emitter: &mut Emitter<W>,
) -> Result<(), ConvertError>;
```

`convert` stops immediately (returns `Ok(())`) once `emitter.limit_reached()` is true. It uses `reader.read_event()`, `BytesStart::name()` compared via `e.local_name()` / `e.name()`, `BytesText::unescape()` for text, and `Attributes` for `text:c` and `outline-level`.

**Behavior per spec mapping table, each with its own test (the incremental checklist):**
- Emit nothing outside `office:body`; emit `office:body`'s *contents* only.
- `office:binary-data`: skip the entire subtree (depth counter), including its text.
- `text:soft-page-break`: skip.
- `text:s`: `emit_spaces(attr text:c or 1)`.
- `text:p`: at the *end* element, `emit_paragraph_break()`. (The C tool emitted at both start and end and relied on a later fix-up pass; we emit once.)
- `text:tab`: `emit_spaces(2)`. `text:line-break`: `emit_line_break()`.
- `text:h`: buffer inner text (spans included, nested elements' text concatenated); on the end element, `emit_heading(outline-level == "1" ? 1 : 2, text)`.
- All other elements: no action (text flows through) — this replaces the C `"<[^>]*>"` strip-everything pass.
- Text events: `emit_text(&e.unescape()?)` — entities (named and numeric) are decoded by the parser, replacing the C 5-entity pass.

Test helper in the same file: `fn convert_str(xml: &str) -> String` — runs `convert` on `Reader::from_reader(Cursor::new(xml))` with a `Vec`-backed emitter (`width: None, subst: None, encoding: UTF_8`, no limits) and returns the output string after `finish()`.

- [ ] **Step 1: Write the failing tests in `src/convert.rs`**

Tests use raw inline XML strings (no zip fixtures — `convert` takes XML directly):
`body_gating`: `<office:document><office:meta>meta text</office:meta><office:body><text:p>Hello</text:p></office:body></office:document>` → output is `"Hello\n"` (finish normalizes).
`paragraph_breaks`: two paragraphs → `"A\n\nB\n"`.
`empty_document`: `<office:document><office:body/></office:document>` → `""` (**Review Focus #3**).
`binary_data_skipped`: paragraph + `office:binary-data` with text inside → binary text absent.
`soft_page_break_skipped`: paragraph containing it → no stray output.
`protected_spaces`: `"<text:p>A<text:s text:c=\"3\"/>B</text:p>"` → `"A   B\n"`.
`tab_and_line_break`: → two spaces and `\n` respectively.
`headings`: `<text:h outline-level="1">Title</text:h>` → `"Title\n=====\n"`; `<text:h><text:span>Sub</text:span></text:h>` → `"Sub\n---\n"`.
`entities_decoded`: `"<text:p>a &amp; b &#65; &#x20AC;</text:p>"` → `"a & b A €\n"` (named predefined + numeric references; the parser decodes them, replacing the C 5-entity pass).
`malformed_xml_is_error`: `convert_str("</unclosed>")` → `Err(ConvertError::Xml(_))` (**Review Focus #2**).

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test convert`
Expected: FAIL — module missing.

- [ ] **Step 3: Implement `convert` in `src/convert.rs`**

Event loop with a small stack of flags: `in_body` (true after the `office:body` start, until its end) and `skip_depth: usize` (incremented on `office:binary-data` start, decremented on its end; text and events ignored while > 0). `text:h` buffering: a `Option<(Vec<u8>, u8)>` holding the pending heading text and level. Write the loop to bail out at the top when `emitter.limit_reached()`.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test convert`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/convert.rs src/lib.rs
git commit -m "feat: convert ODF structure elements via streaming XML parsing"
```

---

### Task 7: Frames and images

**Files:**
- Modify: `src/convert.rs`
- Test: `src/convert.rs` (`#[cfg(test)]`)

**Interfaces:**
- Consumes: `convert` (Task 6), `Emitter::emit_image_marker` (Task 4).
- Produces: extended `convert` behavior — no signature change.

**Behavior:**
- `draw:frame` subtrees are *not* skipped: their text flows through `convert`'s normal rules.
- When a `draw:image` element (or any `draw:*image*`) is encountered inside a frame that has a `draw:name` attribute, emit `[-- Image: NAME --]` at that spot. Track a stack of frame names (push on `draw:frame` start, pop on end); the nearest enclosing named frame wins for nested frames.
- A frame *without* `draw:name` produces no marker, but its text still flows.

- [ ] **Step 1: Write the failing tests in `src/convert.rs`**

`frame_text_flows`: `<draw:frame draw:name="pic1"><draw:text-box><text:p>Caption</text:p></draw:text-box></draw:frame>` inside body → `"Caption\n"`.
`image_marker_at_image`: frame with text box then `<draw:image/>` → `"Caption\n\n[-- Image: pic1 --]\n"`.
`no_name_no_marker`: same but no `draw:name` → `"Caption\n"`, no marker.
`nested_frames_inner_name_wins`: outer `draw:name="outer"`, inner `draw:name="inner"` with the image → marker says `inner`.
`frame_text_without_body_gating`: frames outside `office:body` emit nothing.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test convert`
Expected: FAIL — new tests fail (frames currently emit no markers / flow nothing special).

- [ ] **Step 3: Implement the frame name stack in `src/convert.rs`**

Add `frame_names: Vec<Option<String>>` (push on `draw:frame` start; `None` when `draw:name` absent; pop on end). On `draw:image` (empty or start event), if `frame_names.last()` is `Some(name)`, call `emitter.emit_image_marker(name)`.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test convert`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/convert.rs
git commit -m "feat: flow text through frames and mark images"
```

---

### Task 8: Encoding and codeset detection

**Files:**
- Create: `src/encoding.rs`
- Modify: `src/main.rs` (`--encoding=show` real implementation)
- Test: `src/encoding.rs` (`#[cfg(test)]`)

**Interfaces:**
- Consumes: nothing.
- Produces:

```rust
// src/encoding.rs
pub enum EncodingError { Unknown(String) }

pub fn codeset_from_locale(locale: &str) -> Option<&'static str>; // pure: "en_US.UTF-8" → Some("UTF-8"); no codeset → None
pub fn detect_codeset() -> &'static encoding_rs::Encoding;        // LC_ALL → LC_CTYPE → LANG → UTF_8 default
pub fn resolve_encoding(label: &str) -> Result<&'static encoding_rs::Encoding, EncodingError>;
pub fn transcode(enc: &'static encoding_rs::Encoding, utf8: &[u8]) -> Vec<u8>; // UTF-8 → target bytes; unencodable → b'?'
```

`resolve_encoding` uses `encoding_rs::Encoding::for_label(label.as_bytes())`. `transcode` iterates `utf8` as UTF-8 chars and encodes each with `enc.encode(c.encode_utf8(&mut [0;4]))`; a char the encoding cannot represent becomes `b'?'`.

- [ ] **Step 1: Write the failing tests in `src/encoding.rs`**

`codeset_parsing`: `"en_US.UTF-8"` → `Some("UTF-8")`; `"C"` → `None`; `"de_DE.ISO-8859-15@euro"` → `Some("ISO-8859-15")`.
`resolve`: `"UTF-8"` → `encoding_rs::UTF_8`; `"iso-8859-1"` → `encoding_rs::WINDOWS_1252` (WHATWG maps it); `"not-a-codeset"` → `Err(EncodingError::Unknown(_))`.
`transcode_ascii`: UTF-8 bytes of `"Uber"` → `b"Uber"`.
`transcode_unencodable_becomes_question_mark`: UTF-8 of `"☃"` → `b"?"` when encoding is ASCII (**Review Focus #5**, subst-free half).

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test encoding`
Expected: FAIL — module missing.

- [ ] **Step 3: Implement `src/encoding.rs` and wire `--encoding=show`**

`codeset_from_locale` splits on the last `.` and trims any `@modifier`. `detect_codeset` reads `LC_ALL`, then `LC_CTYPE`, then `LANG` via `std::env::var`, resolving each with `resolve_encoding` until one succeeds, defaulting to `encoding_rs::UTF_8`. In `main.rs`, `Action::ShowEncoding` prints the codeset *name* `detect_codeset().name()` and exits 0.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test encoding`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/encoding.rs src/main.rs src/lib.rs
git commit -m "feat: detect terminal codeset and transcode output"
```

---

### Task 9: The pipeline and golden tests

**Files:**
- Create: `src/lib.rs` (real implementation), `tests/golden.rs`
- Modify: `src/main.rs` (dispatch `Run`)
- Test: `tests/golden.rs`, `tests/cli.rs`

**Interfaces:**
- Consumes: `Config` (Task 2), `open_content_xml` (Task 5), `convert` (Task 6/7), `Emitter` (Task 4), `detect_codeset`/`resolve_encoding`/`transcode` (Task 8).
- Produces:

```rust
// src/lib.rs
pub enum Error {
    Source(SourceError),
    Convert(ConvertError),
    Encoding(EncodingError),
    Io(io::Error),
}

pub fn run(config: Config) -> Result<(), Error>;
```

`run` wires: resolve the target encoding (`config.encoding` else `detect_codeset()`), build `EmitConfig` (`width: (config.width != -1).then_some(config.width as usize)`, `limit_chars`, `deadline: config.limit_time_ms.map(|ms| Instant::now() + Duration::from_millis(ms))`, `subst`, `encoding`), open the source, run `convert`, `finish()` into the UTF-8 bytes, `transcode` to the target encoding, and write to `config.output` (a file, `--output` semantics) or stdout. `--raw` short-circuits: read the raw `content.xml` bytes and write them unprocessed (respecting `--output`). `main()` maps `Err` → `eprintln!("textifyopendocument: {msg}")` + exit 1.

- [ ] **Step 1: Write the failing golden tests in `tests/golden.rs`**

Build fixtures with `tests/support`, run them through `run(Config)` capturing a `Vec<u8>` (an in-process helper `run_to_vec(cfg) -> Result<Vec<u8>, Error>`), and assert the exact output strings:
- `odt_paragraphs_headings`: paragraphs + h1 + h2 → `"First\n\nSecond\n\nTitle\n=====\n\n"` (exact, including finish normalization).
- `ods_cells_flow`: minimal table with two rows of two cells, each cell a paragraph → cell texts separated by `\n\n`, rows concatenated.
- `odp_frame_text`: slide-shaped frame text → text + image marker.
- `raw_flag_prints_xml`: `--raw` → output equals the exact `content.xml` bytes.
- In `tests/cli.rs`: run the compiled binary on a temp ODT fixture (`tests/support::write_temp_file`), assert stdout equals the golden string and exit code 0; run it on a garbage file, assert exit code 1 and stderr mentions the filename.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test`
Expected: FAIL — `run` and `run_to_vec` missing.

- [ ] **Step 3: Implement `run` and the `main` dispatch**

As described in Interfaces. `emit` writes into a `Vec<u8>`; `finish()` returns it. `transcode` is skipped entirely when the target encoding is UTF-8 (pass the bytes through).

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test`
Expected: PASS (all earlier unit tests included)

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs src/main.rs tests/golden.rs tests/cli.rs tests/support/mod.rs
git commit -m "feat: wire the conversion pipeline end to end"
```

---

### Task 10: Limits end to end and performance smoke

**Files:**
- Create: `tests/perf.rs`
- Test: `tests/perf.rs` (marked `#[ignore]`), plus two fast limit tests in `tests/cli.rs`

**Interfaces:**
- Consumes: the compiled binary, `tests/support` (Task 5).

- [ ] **Step 1: Write the failing fast limit tests in `tests/cli.rs`**

`limit_chars_truncates`: run the binary with `--limit-chars=5` on a fixture whose full text is `"abcdefgh"` in one paragraph → stdout has exactly 5 chars, exit 0.
`limit_time_truncates`: run with `--limit-time=1` on a moderately large generated fixture → exit 0; assert elapsed wall time < 5 s and stdout length < the full-output length.

- [ ] **Step 2: Write the `#[ignore]` performance test in `tests/perf.rs`**

`big_spreadsheet_full_mode_and_preview`: generate an ODS in code (20,000 rows × 4 columns, each cell a paragraph `"cell {row}-{col}"`; write to a temp file); assert full mode completes in under 60 s and the output contains `"cell 19999-3"`; assert `--limit-chars=5000` output has ≤ 5000 chars and completes in under 10 s.

- [ ] **Step 3: Run to verify they fail**

Run: `cargo test --test cli limit_`
Expected: FAIL — limits not yet wired through `main` (or behavior wrong).

- [ ] **Step 4: Implement any missing wiring**

The pipeline already handles limits (Tasks 4/6/9); fix whatever the failing tests reveal — most likely `main` not passing `limit_chars`/`limit_time_ms` into `run`, or `convert` not checking `limit_reached()` between events.

- [ ] **Step 5: Run all tests, then the ignored perf test**

Run: `cargo test` (PASS), then `cargo test --test perf -- --ignored`
Expected: PASS, with the perf test taking seconds, not minutes.

- [ ] **Step 6: Commit**

```bash
git add tests/cli.rs tests/perf.rs
git commit -m "test: cover truncation limits and large-spreadsheet performance"
```

---

### Task 11: Documentation

**Files:**
- Create: `textifyopendocument.1` (man page, groff)
- Modify: `README.md`, `src/main.rs` (final `--help` text)

- [ ] **Step 1: Write the man page**

`textifyopendocument.1` with SYNOPSIS `textifyopendocument [OPTIONS] FILENAME`, DESCRIPTION (fast OpenDocument→plain text for previews, reimagining of odt2txt), OPTIONS matching Global Constraints exactly (including `--limit-chars`, `--limit-time`), and an attribution note: "Reimagines odt2txt by Dennis Stosberg" with the upstream URL.

- [ ] **Step 2: Update `README.md`**

Usage examples, the full flag list, the ranger snippet:

```
textifyopendocument --limit-chars=5000 "${FILE_PATH}" && exit 5
```

and a note that the Final Boss spreadsheet in `acceptance-tests/` is the acceptance gate.

- [ ] **Step 3: Finalize `--help` in `src/main.rs`**

The usage string lists every flag from Global Constraints with one-line descriptions matching the man page.

- [ ] **Step 4: Verify**

Run: `cargo run -- --help` — eyeball that every flag appears and matches the man page. If `groff` is installed, render the man page with `groff -Tascii -man textifyopendocument.1` and check it renders without errors; otherwise skip rendering.

- [ ] **Step 5: Commit**

```bash
git add textifyopendocument.1 README.md src/main.rs
git commit -m "docs: man page, README usage, and final --help text"
```

---

### Final acceptance (manual, after all tasks)

1. Run `cargo build --release`.
2. When the Human has placed the Final Boss spreadsheet in `acceptance-tests/`, run
   `time ./target/release/textifyopendocument acceptance-tests/<file>.ods | head` (full mode) and
   `time ./target/release/textifyopendocument --limit-chars=5000 acceptance-tests/<file>.ods` (preview mode).
3. Expected: full mode completes in seconds (the Human is "wait and see" on the exact number), preview mode is bounded by the limit, output is sane flowing text. Compare the same command against `/usr/bin/odt2txt` for a dramatic side-by-side.
