/*
 * textifyopendocument: a fast converter from OpenDocument to plain text.
 *
 * Copyright (c) 2026 jbrains
 *
 * This program is free software; you can redistribute it and/or
 * modify it under the terms of the GNU General Public License,
 * version 2 as published by the Free Software Foundation
 */

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum SubstMode {
    None,
    Some,
    All,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub input: PathBuf,
    pub raw: bool,
    pub raw_input: bool,
    pub width: i64, // default 65; -1 = no wrap; other values < 3 invalid
    pub output: Option<PathBuf>,
    pub encoding: Option<String>,
    pub subst: SubstMode, // default SubstMode::Some
    pub limit_chars: Option<usize>,
    pub limit_time_ms: Option<u64>,
}

#[derive(Debug, PartialEq)]
pub enum Action {
    Run(Config),
    ShowEncoding,
    Help,
    Version,
}

#[derive(Debug, PartialEq)]
pub enum CliError {
    MissingFilename,
    UnknownOption(String),
    InvalidWidth(String),
    InvalidSubst(String),
    InvalidLimit(String),
}

pub fn parse_args(args: &[String]) -> Result<Action, CliError> {
    let mut cfg = Config {
        input: PathBuf::new(),
        raw: false,
        raw_input: false,
        width: 65,
        output: None,
        encoding: None,
        subst: SubstMode::Some,
        limit_chars: None,
        limit_time_ms: None,
    };
    let mut seen_filename = false;

    for arg in args {
        if let Some(value) = arg.strip_prefix("--width=") {
            let width = value
                .parse::<i64>()
                .map_err(|_| CliError::InvalidWidth(value.to_string()))?;
            if width < 3 && width != -1 {
                return Err(CliError::InvalidWidth(value.to_string()));
            }
            cfg.width = width;
        } else if let Some(value) = arg.strip_prefix("--output=") {
            if value != "-" {
                cfg.output = Some(PathBuf::from(value));
            }
        } else if let Some(value) = arg.strip_prefix("--encoding=") {
            if value == "show" {
                return Ok(Action::ShowEncoding);
            }
            cfg.encoding = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("--subst=") {
            cfg.subst = match value {
                "none" => SubstMode::None,
                "some" => SubstMode::Some,
                "all" => SubstMode::All,
                other => return Err(CliError::InvalidSubst(other.to_string())),
            };
        } else if let Some(value) = arg.strip_prefix("--limit-chars=") {
            let n = value
                .parse::<usize>()
                .map_err(|_| CliError::InvalidLimit(value.to_string()))?;
            cfg.limit_chars = Some(n);
        } else if let Some(value) = arg.strip_prefix("--limit-time=") {
            let n = value
                .parse::<u64>()
                .map_err(|_| CliError::InvalidLimit(value.to_string()))?;
            cfg.limit_time_ms = Some(n);
        } else if arg == "--raw" {
            cfg.raw = true;
        } else if arg == "--raw-input" {
            cfg.raw_input = true;
        } else if arg == "--force" {
            // accepted and ignored, as the C tool does
        } else if arg == "--help" {
            return Ok(Action::Help);
        } else if arg == "--version" {
            return Ok(Action::Version);
        } else if arg.starts_with("--") {
            return Err(CliError::UnknownOption(arg.clone()));
        } else if !seen_filename {
            cfg.input = PathBuf::from(arg);
            seen_filename = true;
        } else {
            return Err(CliError::UnknownOption(arg.clone()));
        }
    }

    if !seen_filename {
        return Err(CliError::MissingFilename);
    }
    Ok(Action::Run(cfg))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    fn default_config() -> Config {
        Config {
            input: PathBuf::from("doc.odt"),
            raw: false,
            raw_input: false,
            width: 65,
            output: None,
            encoding: None,
            subst: SubstMode::Some,
            limit_chars: None,
            limit_time_ms: None,
        }
    }

    #[test]
    fn parse_defaults() {
        assert_eq!(
            parse_args(&s(&["doc.odt"])),
            Ok(Action::Run(default_config()))
        );
    }

    #[test]
    fn parse_each_flag() {
        let flags: Vec<(&str, Box<dyn Fn(&Config) -> bool>)> = vec![
            ("--width=80", Box::new(|c: &Config| c.width == 80)),
            ("--width=-1", Box::new(|c: &Config| c.width == -1)),
            ("--raw", Box::new(|c: &Config| c.raw)),
            ("--raw-input", Box::new(|c: &Config| c.raw_input)),
            (
                "--output=/tmp/out.txt",
                Box::new(|c: &Config| c.output == Some(PathBuf::from("/tmp/out.txt"))),
            ),
            ("--output=-", Box::new(|c: &Config| c.output == None)),
            (
                "--encoding=UTF-8",
                Box::new(|c: &Config| c.encoding.as_deref() == Some("UTF-8")),
            ),
            ("--subst=none", Box::new(|c: &Config| c.subst == SubstMode::None)),
            ("--subst=all", Box::new(|c: &Config| c.subst == SubstMode::All)),
            (
                "--limit-chars=5000",
                Box::new(|c: &Config| c.limit_chars == Some(5000)),
            ),
            (
                "--limit-time=200",
                Box::new(|c: &Config| c.limit_time_ms == Some(200)),
            ),
            ("--force", Box::new(|_: &Config| true)),
        ];
        for (flag, check) in flags {
            let cfg = match parse_args(&s(&[flag, "doc.odt"])) {
                Ok(Action::Run(cfg)) => cfg,
                other => panic!("expected Run for {}, got {:?}", flag, other),
            };
            assert!(check(&cfg), "flag {} did not take effect: {:?}", flag, cfg);
        }
    }

    #[test]
    fn parse_errors() {
        assert_eq!(parse_args(&s(&[])), Err(CliError::MissingFilename));
        assert_eq!(
            parse_args(&s(&["--width=2"])),
            Err(CliError::InvalidWidth("2".into()))
        );
        assert_eq!(
            parse_args(&s(&["--subst=sometimes"])),
            Err(CliError::InvalidSubst("sometimes".into()))
        );
        assert_eq!(
            parse_args(&s(&["--limit-chars=abc"])),
            Err(CliError::InvalidLimit("abc".into()))
        );
        assert!(matches!(
            parse_args(&s(&["a", "b"])),
            Err(CliError::UnknownOption(_))
        ));
        assert!(matches!(
            parse_args(&s(&["--bogus"])),
            Err(CliError::UnknownOption(_))
        ));
    }

    #[test]
    fn parse_show_encoding() {
        assert_eq!(parse_args(&s(&["--encoding=show"])), Ok(Action::ShowEncoding));
        assert_eq!(parse_args(&s(&["--help"])), Ok(Action::Help));
        assert_eq!(parse_args(&s(&["--version"])), Ok(Action::Version));
    }
}
