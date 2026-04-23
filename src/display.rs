//! Utilities for displaying things to the user.

use std::fmt;

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
        if arg.chars().any(needs_quoting) {
            write!(f, "{arg:?}")
        } else {
            write!(f, "{arg}")
        }
    })
}
