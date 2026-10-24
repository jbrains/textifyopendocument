# textifyopendocument

A fast converter from OpenDocument files (text, spreadsheets, presentations)
to plain text, designed with file-manager previews in mind.

This project began as a speed-focused reimagining of
[odt2txt](https://github.com/dstosberg/odt2txt) by Dennis Stosberg. The
original processes OpenDocument files with regular expressions, which becomes
extremely slow on large documents. textifyopendocument replaces the
regular-expression pipeline with streaming XML parsing: one linear pass
instead of ~55 quadratic ones. A 20,000-row spreadsheet that takes the
original over 15 minutes converts in well under a second.

## Installation

From crates.io:

```
cargo install textifyopendocument
```

From source:

```
git clone https://codeberg.org/jbrains/textifyopendocument
cd textifyopendocument
cargo build --release        # binary at target/release/textifyopendocument
```

Arch Linux (from the PKGBUILD in this repo; AUR submission pending):

```
git clone https://codeberg.org/jbrains/textifyopendocument
cd textifyopendocument/packaging/aur
makepkg -si
```

Nix:

```
nix run git+https://codeberg.org/jbrains/textifyopendocument
```

Homebrew:

```
brew tap jbrains/homebrew-tap https://codeberg.org/jbrains/homebrew-tap
brew install textifyopendocument
```

## Usage

```
textifyopendocument [OPTIONS] FILENAME
```

Basic conversion prints plain text to standard output:

```
textifyopendocument document.odt
```

Truncated preview mode stops converting once a limit is hit — useful for
preview panes that only need the first part of the output:

```
textifyopendocument --limit-chars=5000 big-spreadsheet.ods
textifyopendocument --limit-time=200 big-spreadsheet.ods
```

### Options

| Option | Meaning |
|---|---|
| `--width=N` | Wrap lines after N characters (default 65; `-1` disables wrapping) |
| `--output=FILE` | Write to FILE instead of stdout |
| `--raw` | Print the raw content.xml |
| `--raw-input` | Input is a raw XML file (fodt, fods, ...) |
| `--encoding=X` | Convert output to encoding X; `--encoding=show` prints the detected one |
| `--subst=none\|some\|all` | ASCII look-alike substitution policy (default `some`) |
| `--limit-chars=N` | Stop after N output characters |
| `--limit-time=N` | Stop after N milliseconds |
| `--force` | Accepted for compatibility; ignored |
| `--version`, `--help` | Version and usage |

### ranger integration

`ranger` launches the tool with just the filename. To give its preview pane
the truncated mode, use a line like:

```
textifyopendocument --limit-chars=5000 "${FILE_PATH}" && exit 5
```

## Licence

GPL, version 2. See LICENSE.

## Acknowledgements

* Dennis Stosberg, author of odt2txt, whose tool and his advice to replace
  the regular-expression approach with XML parsing inspired this project.
