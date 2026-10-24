/*
 * textifyopendocument: a fast converter from OpenDocument to plain text.
 *
 * Copyright (c) 2026 jbrains
 *
 * This program is free software; you can redistribute it and/or
 * modify it under the terms of the GNU General Public License,
 * version 2 as published by the Free Software Foundation
 */

use textifyopendocument::args::{self, Action};

fn main() {
    let argv: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    match args::parse_args(&argv) {
        Ok(Action::Help) => {
            print!(
                "textifyopendocument 0.1.0\n\
                 Converts OpenDocument files (text, spreadsheets, presentations) to plain text.\n\n\
                 Syntax:   textifyopendocument [OPTIONS] FILENAME\n\n\
                 Options:  --width=N       Wrap lines after N characters. Default: 65.\n\
                 \x20                          Use -1 to disable wrapping.\n\
                 \x20         --output=FILE   Write output to FILE instead of stdout\n\
                 \x20         --raw           Print the raw content.xml\n\
                 \x20         --raw-input     Input is a raw XML file (fodt, fods, ...)\n\
                 \x20         --encoding=X    Convert output to encoding X.\n\
                 \x20                         --encoding=show prints the encoding that would be used\n\
                 \x20         --subst=X       Replace non-ascii characters by ascii look-a-likes:\n\
                 \x20                         all | some (default) | none\n\
                 \x20         --limit-chars=N Truncated preview: stop after N output characters\n\
                 \x20         --limit-time=N  Truncated preview: stop after N milliseconds\n\
                 \x20         --force         Accepted for compatibility with odt2txt; ignored\n\
                 \x20         --version       Show version information\n\
                 \x20         --help          Show this help\n"
            );
            std::process::exit(0);
        }
        Ok(Action::Version) => {
            println!("textifyopendocument 0.1.0");
            std::process::exit(0);
        }
        Ok(Action::ShowEncoding) => {
            println!("{}", textifyopendocument::encoding::detect_codeset().name());
            std::process::exit(0);
        }
        Ok(Action::Run(config)) => {
            if let Err(err) = textifyopendocument::run(&config) {
                eprintln!("textifyopendocument: {}", err);
                std::process::exit(1);
            }
        }
        Err(err) => {
            eprintln!("textifyopendocument: {:?}", err);
            std::process::exit(1);
        }
    }
}
