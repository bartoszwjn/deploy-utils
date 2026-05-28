//! Utilities for displaying things to the user.

use std::fmt;

use unicode_width::UnicodeWidthStr;

/// ANSI text styles used for displaying elements.
pub(crate) mod styles {
    use anstyle::{AnsiColor, Style};

    // Section headers.
    pub(crate) const HEADER: Style = Style::new().bold();

    // `ProfileInfo` elements.
    pub(crate) const NODE: Style = AnsiColor::Blue.on_default();
    pub(crate) const PROFILE: Style = AnsiColor::Cyan.on_default();
    pub(crate) const USER: Style = AnsiColor::Yellow.on_default();
    pub(crate) const SUDO: Style = AnsiColor::Magenta.on_default();
    pub(crate) const INTERACTIVE_SUDO: Style = AnsiColor::Red.on_default();
    pub(crate) const SSH_USER: Style = AnsiColor::Yellow.on_default();
    pub(crate) const HOSTNAME: Style = AnsiColor::Green.on_default();
    pub(crate) const PATH: Style = AnsiColor::Blue.on_default();
    pub(crate) const FAST: Style = AnsiColor::Red.on_default();
    pub(crate) const SSH_OPTS: Style = AnsiColor::Cyan.on_default();

    pub(crate) const SUCCESS: Style = AnsiColor::Green.on_default();
    pub(crate) const FAILURE: Style = AnsiColor::Red.on_default();
    pub(crate) const WARNING: Style = AnsiColor::Yellow.on_default();
    pub(crate) const UNKNOWN: Style = AnsiColor::BrightBlack.on_default();
}

pub(crate) fn get_max_width(elements: impl IntoIterator<Item = impl AsRef<str>>) -> usize {
    elements
        .into_iter()
        .map(|elem| UnicodeWidthStr::width(elem.as_ref()))
        .max()
        .unwrap_or(0)
}

pub(crate) fn display_command_args<Iter>(make_args: impl Fn() -> Iter) -> impl fmt::Display
where
    Iter: Iterator,
    Iter::Item: AsRef<str>,
{
    fmt::from_fn(move |f| {
        let mut first = true;
        for arg in make_args() {
            let arg = display_command_arg(arg.as_ref());
            if first {
                write!(f, "{arg}")?;
                first = false;
            } else {
                write!(f, " {arg}")?;
            }
        }
        Ok(())
    })
}

fn display_command_arg(arg: &str) -> impl fmt::Display {
    fn needs_quoting(c: char) -> bool {
        match c {
            '"' | '\'' | '\\' => true,
            _ if c.is_whitespace() => true,

            _ if c.is_alphanumeric() => false,
            _ if c.is_ascii_punctuation() => false,

            _ => true,
        }
    }

    fmt::from_fn(move |f| {
        if arg.is_empty() || arg.chars().any(needs_quoting) {
            write!(f, "{arg:?}")
        } else {
            write!(f, "{arg}")
        }
    })
}

pub(crate) fn indent(amount: usize, s: &str) -> impl fmt::Display {
    fmt::from_fn(move |f| {
        let mut lines = s.lines().peekable();
        while let Some(line) = lines.next() {
            if lines.peek().is_some() {
                writeln!(f, "{:amount$}{line}", "")?;
            } else {
                write!(f, "{:amount$}{line}", "")?;
            }
        }
        Ok(())
    })
}
