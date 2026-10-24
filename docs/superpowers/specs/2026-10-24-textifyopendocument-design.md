# textifyopendocument — Design

Date: 2026-10-24
Status: proposed

## Intent and why

The C tool `odt2txt` (by Dennis Stosberg) converts OpenDocument files to plain
text using regular expressions. On large documents it becomes pathologically
slow: the Human's Final Boss spreadsheet (4 tabs, ~20,000 rows, up to 40
columns) takes over 15 minutes. `ranger` launches `odt2txt "${FILE_PATH}"` in
the background to build file previews, so a single large spreadsheet holds
the machine's resources hostage for a quarter of an hour.

The original author has stated that the regular-expression approach is doomed
to failure on larger documents and suggests an XML-parser-based approach.

`textifyopendocument` is a new Rust tool that replaces the regex pipeline with
streaming XML parsing. It is a helpful alternative, **not a drop-in
replacement**: the goal is clean, sensible plain text for previews, not
byte-for-byte output compatibility with the C tool.

## Goals

1. Dramatically faster conversion: one streaming, roughly O(n) pass instead
   of ~55 full-buffer regex passes with quadratic memmoves.
2. Two modes:
   - **Full text mode** (default): render all the text. The Human takes a
     "wait and see" attitude on performance here; the expectation is that
     replacing regexes with XML parsing already makes it acceptable.
   - **Truncated preview mode**: the caller passes a character limit or a
     processing-time limit; the tool stops early and does not convert the
     entire file. This is the mode `ranger` will eventually use.
3. Sensible plain-text output. The community that adopts this tool influences
   how edge cases should behave.
4. Drop-in *invocation* compatibility for the common case: `tool FILE` prints
   the text to stdout. `ranger` passes no arguments.
5. Attribution culture: the project credits the original `odt2txt` work and
   the Human's work (see Constraints).

## Non-goals

- Byte-identical output with the C `odt2txt`. Quirks and bugs of the C
  version are not replicated unless they are also the sensible behavior.
- A full ODF renderer. No formatting fidelity: no styles, no fonts, no
  layout.
- Table structure in the output (for now). A spreadsheet renders as its
  text flowing continuously, as the C tool does. *See Future ideas.*
- Windows support (for now). *See Constraints.*

## Anti-goals (failures even if it technically works)

- Still taking minutes on the Final Boss spreadsheet.
- Breaking `ranger` integration: it must remain trivially invocable.
- Losing the attribution story (GPL-2 + acknowledgements).
- Scope creep into table structure, formatting fidelity, or document
  *writing*.

## Constraints

- **Language: Rust.** Explicit goal of the project.
- **Approach: XML parsing replaces regular expressions.** The original
  author's stated direction, adopted by the Human as the core strategy.
- **License: GPL-2** (matching the original). The rewrite strategy — studying
  the C version regex by regex and reimplementing its behavior — makes this a
  derivative work of GPL-2-only code, so GPL-2 is the legally safe choice and
  honors the original author's choice. Attribution is handled culturally:
  `AUTHORS` file, README acknowledgements, man-page mention, and preserved
  copyright notices wherever code is copied. (Already in place in the repo.)
- **Platform: Linux only.** macOS is nice-to-have.

  > **EXPENSIVE TO REVERSE.** This decision is cheap to make and expensive to
  > change later. If the tool gains wide adoption, platform support should be
  > revisited as a deliberate, documented decision. Concrete consequences to
  > keep in mind: file-manager integration (ranger) is Unix-only; the
  > encoding/terminal-detection logic must not hard-code glibc; CI should be
  > structured so adding platforms later is mechanical. Do not remove the
  > original's portability knowledge from reach — the C reference remains in
  > this repository.

- **Repository layout:** new git repository `textifyopendocument`. The
  original `odt2txt` sources live in `odt2txt/` as a plain copy (no git
  history), kept as reference; the snapshot commit is recorded in
  `odt2txt/SNAPSHOT.md`. `acceptance-tests/` holds
  hand-supplied real-world documents (the Final Boss), which are gitignored,
  not committed.
- **Acceptance testing:** the Human supplies exactly one acceptance file (the
  Final Boss spreadsheet). The implementer must generate all other test
  files. The Final Boss is the gate; generated files are the safety net.
- **Incremental strategy (decided by the Human):** for each regular
  expression the C version uses: (1) implement the corresponding behavior in
  the Rust version with XML parsing; (2) look for opportunities to simplify —
  fewer passes, or ideas that needed separate regexes but don't need separate
  processing with an XML parser; (3) repeat until done.

## The C pipeline and why it is slow (reference analysis)

The C tool reads `content.xml` from the zip, then runs, in order:

1. `subst_doc`: up to 36 passes, one regex substitution per Unicode
   look-alike character.
2. `format_doc`: 17 regex passes (strip everything before `<office:body>`,
   drop binary data, soft page breaks, protected spaces, headings, paragraphs,
   tabs, line breaks, image frames, "remove all remaining tags", collapse
   indentation/newlines, decode 5 entities, trim document ends).
3. `wrap`: line wrapping at the width option (default 63 in code / 65 in
   docs).
4. One more regex pass removing trailing spaces.
5. iconv conversion to the terminal encoding.

Every substitution is a `memmove` of the entire remaining buffer, so the
whole pipeline is effectively O(n²) in the number of matches — hence 15+
minutes on a large spreadsheet.

## Regex → XML-parser behavior mapping (the core of the strategy)

The following table is the working document for the rewrite. Each C regex is
replaced by an event in a single streaming parse. The right-hand column
describes the *sensible* behavior, including deliberate simplifications and
improvements over the C version.

| C regex | Streaming XML behavior | Notes |
|---|---|---|
| `.*<office:body>` → keep only body | Start emitting only inside `office:body` (needed for `--raw-input` too) | C regex is a catastrophic-backtracking candidate on big files |
| `<office:binary-data>...</office:binary-data>` → "" | Skip the `office:binary-data` subtree | |
| `<text:soft-page-break/>` → "" | Skip `text:soft-page-break` elements | |
| `<text:s/>` → " " | On `text:s`, emit `text:c` spaces (default 1) | C ignores the `text:c` count; honoring it is a sensible improvement |
| `<text:h outline-level="1">X</text:h>` → `X` underlined with `=` | On `text:h` with `outline-level="1"`, emit `X\n===...\n\n` | Underline keeps the C tool's useful preview convention |
| `<text:h>X</text:h>` → `X` underlined with `-` | Other `text:h`: emit `X\n---...\n\n` | |
| `<text:p [^>]*>` → "\n\n" **and** `</text:p>` → "\n\n" | On `text:p` end, emit one blank line (`\n\n`) | C emits at both start and end and relies on a later `\n{3,}`→`\n\n` pass to fix the double-emission; the parser emits once. Note: C only matched `<text:p ` *with a literal space* — a bug not replicated |
| `<text:tab/>` → "  " | On `text:tab`, emit two spaces | |
| `<text:line-break/>` → "\n" | On `text:line-break`, emit `\n` | |
| `<draw:frame draw:name="N">…` → `[-- Image: N --]` | Do **not** skip `draw:frame` subtrees: emit any text inside them normally; when an image element sits inside a named frame, emit `[-- Image: N --]` at that spot | Frames contain text boxes — in `.odp`, nearly all slide text lives there. C replaced only the start tag and leaked the subtree through other rules |
| `<[^>]*>` → "" (strip all tags) | Not needed at all — the parser never emits markup | The biggest single win |
| `&apos;` `&amp;` `&quot;` `&gt;` `&lt;` | The XML parser decodes all entities, including numeric references | Strict improvement: C decoded only 5 entities |
| `\n +` → "\n" (strip indentation) | Maintain "at start of line" state; drop whitespace there | Cheap boolean flag instead of a pass |
| `\n{3,}` → "\n\n" | Cap consecutive newlines at 2 while emitting | Counter instead of a pass |
| `^\n+` → "" | Emit nothing until the first non-newline output | |
| `\n{2,}$` → "\n" | At end of output, ensure at most one trailing newline | |
| ` +\n` → "\n" (trailing spaces) | Strip trailing spaces per line while emitting (or during wrap) | |
| 36 Unicode look-alike substitutions | Char-by-char mapping table applied to emitted text (see Appendix) | O(1) per char instead of 36 full-buffer passes. Fix the C table's duplicate/bogus bullet entry (0x2023) |
| `wrap()` (width, default 65, `-1` = off) | Streaming line wrapper at emission or as a single line pass | Unicode-aware; keep default 65 (the documented default; the C code's 63 is a bug) |
| iconv conversion | encoding_rs conversion, applied streaming | See "Encoding" below |

Spreadsheets specifically: text lives in `table:table-cell` elements
containing `text:p` paragraphs; the paragraph rule above yields the current
"flowing text" output. No cell separators for now (recorded as a future
idea).

## Architecture sketch

Single streaming pipeline; no document tree is built:

1. **Open input.** `.odt`/`.ods`/`.odp` → `zip` crate, locate `content.xml`,
   stream-decompress it. `--raw-input` → read the file directly.
2. **Parse.** `quick-xml` pull parser feeds events into a small state
   machine implementing the mapping table above. Entities are decoded by the
   parser. DTDs are not resolved (ODF content.xml does not need them).
3. **Emit.** The state machine writes to an output buffer, applying the
   substitution table, newline capping, and line wrapping as text flows.
4. **Convert encoding.** `encoding_rs` streams the buffer to the target
   encoding. UTF-8 targets are a no-op copy (the overwhelmingly common case).
5. **Write.** stdout or `--output=FILE`.

Because every stage is streaming, truncation is natural:

- **`--limit-chars=N`** (new): stop emitting once N characters of output have
  been produced, then finalize and exit. No further parsing happens. N counts
  Unicode scalar values of the final text, newlines included (not bytes, not
  grapheme clusters).
- **`--limit-time=N`** (new): deadline of N milliseconds; check the clock
  periodically (e.g., every chunk of events/bytes) and stop early. If both
  limits are given, whichever fires first wins.

A pull parser makes both limits cheap: stop consuming events and drop the
reader.

## CLI surface

Kept flags (invocation-compatible where it matters; semantics may be
simplified): `--width=N` (default 65, `-1` disables wrapping), `--output=FILE`,
`--raw` (print the raw content.xml), `--raw-input`, `--encoding=X` (also
`--encoding=show`), `--subst=none|some|all`, `--version`, `--help`, and
`--force` (accepted and ignored, as the C tool does). New flags:
`--limit-chars=N`, `--limit-time=N`. Binary name: `textifyopendocument`.

`ranger` integration stays one line; adopting preview limits later means
editing the user's scope line to
`textifyopendocument --limit-chars=5000 "${FILE_PATH}" && exit 5`
(or similar).

## Encoding

- Default: detect the terminal codeset from the locale (LC_ALL, LC_CTYPE,
  LANG). UTF-8 (the common case) means no conversion work.
- `--encoding=X`: convert to X via encoding_rs. If X is not available,
  warn and emit UTF-8.
- `--subst`: the look-alike table is applied for characters the target
  encoding cannot represent (`some`, the default) or always (`all`), or never
  (`none`). This replaces the C tool's 36 iconv-probe regex passes with a
  per-character lookup.

## Testing and acceptance

The implementer generates all test files. Approach:

- **Unit tests** per behavior mapping: feed crafted XML fragments through the
  state machine; assert exact output. This is where the regex-by-regex
  incremental strategy lives.
- **Fixture generation in test code:** build minimal `.odt`/`.ods`/`.odp`
  zips programmatically (the `zip` crate can write), so tests are
  self-contained.
- **Golden tests:** full-document fixtures with expected output text.
- **Differential spot checks (development aid, not CI):** compare against
  `/usr/bin/odt2txt` on ordinary ODT documents where sensible behavior and C
  behavior should agree. Skips cleanly when the C binary is absent.
- **Performance smoke test:** generate a large spreadsheet fixture
  (~20k rows) in-test; assert full mode completes within a generous
  CI-safe bound and that preview mode truncates correctly. The real
  Final Boss remains the acceptance gate.
- **Final Boss acceptance:** when the Human drops the spreadsheet into
  `acceptance-tests/`, run full mode and preview mode, report wall time and
  confirm sane output. Full mode target: seconds, not minutes (the Human is
  "wait and see" on the exact number); preview mode: bounded by the caller's
  limit.

## Repository layout

```
textifyopendocument/
  Cargo.toml, src/, tests/
  acceptance-tests/          # Final Boss goes here (gitignored)
  docs/superpowers/specs/    # this document
  odt2txt/                   # C reference: plain copy, snapshot in SNAPSHOT.md
  AUTHORS, README.md, LICENSE  # already in place
```

## Appendix: look-alike substitution table

Extracted from the C source (`odt2txt.c`, `substs[]`). The C tool inserts
HTML-entity-encoded strings (`&lt;`, `&apos;`, …) and relies on a *later*
pass to decode them; this design emits the decoded text directly — one more
pass eliminated. The C table's entry for U+2023 mistakenly repeats the
U+2022 codepoint; it is corrected here.

| Codepoint | Char | Emitted text |
|---|---|---|
| U+00A0 | no-break space | ` ` |
| U+00A9 | © | `(c)` |
| U+00AB | « | `<<` |
| U+00AD | soft hyphen | `-` |
| U+00AE | ® | `(r)` |
| U+00BB | » | `>>` |
| U+00BC | ¼ | `1/4` |
| U+00BD | ½ | `1/2` |
| U+00BE | ¾ | `3/4` |
| U+00C4 | Ä | `Ae` |
| U+00D6 | Ö | `Oe` |
| U+00DC | Ü | `Ue` |
| U+00DF | ß | `ss` |
| U+00E4 | ä | `ae` |
| U+00F6 | ö | `oe` |
| U+00FC | ü | `ue` |
| U+2010 | ‐ | `-` |
| U+2011 | ‑ | `-` |
| U+2012 | ‒ | `-` |
| U+2013 | – | `-` |
| U+2014 | — | `--` |
| U+2015 | ― | `--` |
| U+2018 | ' | `` ` `` |
| U+2019 | ' | `'` |
| U+201A | ‚ | `,` |
| U+201B | ‛ | `` ` `` |
| U+201C | " | ```` `` ```` |
| U+201D | " | `''` |
| U+201E | „ | `,,` |
| U+2022 | • | `o ` |
| U+2023 | ‣ | `< ` |
| U+2025 | ‥ | `..` |
| U+2026 | … | `...` |
| U+2030 | ‰ | `o/oo` |
| U+2039 | ‹ | `<` |
| U+203A | › | `>` |
| U+20AC | € | `EUR` |
| U+2190 | ← | `<-` |
| U+2192 | → | `->` |
| U+2194 | ↔ | `<->` |

## Future ideas (recorded, not in scope)

- **Table structure in previews:** add clear boundaries between spreadsheet
  rows (and possibly column separation). Explicitly requested by the Human to
  be recorded for future work.
- A ready-made `ranger` scope snippet shipping with the tool.
- macOS port (see the EXPENSIVE TO REVERSE note above).

## Left to the builder (calls made by the implementer)

- Exact crate choices and versions (proposed: `quick-xml`, `zip`,
  `encoding_rs`; no CLI framework needed for this small, fixed flag set).
- Module structure and code organization.
- Deadline-check granularity for `--limit-time`.
- Error messages and exit codes.
- Details of the streaming line wrapper's interplay with the truncation
  limits.
