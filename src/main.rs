/*
 * textifyopendocument: a fast converter from OpenDocument to plain text.
 *
 * Copyright (c) 2026 jbrains
 *
 * This program is free software; you can redistribute it and/or
 * modify it under the terms of the GNU General Public License,
 * version 2 as published by the Free Software Foundation
 */

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--version") {
        println!("textifyopendocument 0.1.0");
        std::process::exit(0);
    }
    if args.first().map(String::as_str) == Some("--help") {
        println!("usage: textifyopendocument [OPTIONS] FILENAME");
        std::process::exit(0);
    }
    std::process::exit(0);
}
