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
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match args::parse_args(&argv) {
        Ok(Action::Help) => {
            println!("usage: textifyopendocument [OPTIONS] FILENAME");
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
        Ok(Action::Run(_config)) => {
            eprintln!("textifyopendocument: not yet implemented");
            std::process::exit(1);
        }
        Err(err) => {
            eprintln!("textifyopendocument: {:?}", err);
            std::process::exit(1);
        }
    }
}
