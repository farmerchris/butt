use crate::cli::HighlightColor;
use regex::Regex;
use std::io::{self, IsTerminal};

pub(crate) fn decorate_line(
    line: &str,
    regex: Option<&Regex>,
    color: &HighlightColor,
    colors_enabled: bool,
) -> String {
    if let Some(rgx) = regex {
        if colors_enabled {
            highlight_matches(line, rgx, color)
        } else {
            line.to_string()
        }
    } else {
        line.to_string()
    }
}

pub(crate) fn highlight_matches(line: &str, regex: &Regex, color: &HighlightColor) -> String {
    let mut out = String::with_capacity(line.len());
    let mut last = 0;
    for mat in regex.find_iter(line) {
        out.push_str(&line[last..mat.start()]);
        out.push_str(&color.paint(mat.as_str()));
        last = mat.end();
    }
    out.push_str(&line[last..]);
    out
}

pub(crate) fn should_use_color() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }

    if let Some(force) = std::env::var_os("CLICOLOR_FORCE")
        && force.to_string_lossy() != "0"
    {
        return true;
    }

    if let Some(clicolor) = std::env::var_os("CLICOLOR")
        && clicolor.to_string_lossy() == "0"
    {
        return false;
    }

    if std::env::var("TERM").is_ok_and(|term| term == "dumb") {
        return false;
    }

    io::stdout().is_terminal()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::HighlightColor;

    #[test]
    fn highlights_all_matches() {
        let re = Regex::new("ERR").expect("regex should compile");
        let out = highlight_matches("x ERR y ERR z", &re, &HighlightColor::Red);
        assert!(out.contains("\x1b[31mERR\x1b[0m"));
        assert_eq!(out.matches("\x1b[31mERR\x1b[0m").count(), 2);
    }

    #[test]
    fn decorates_plain_when_no_regex() {
        let out = decorate_line("plain text", None, &HighlightColor::Yellow, true);
        assert_eq!(out, "plain text");
    }
}
