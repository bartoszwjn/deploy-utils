//! Utilities for working with Nix.

use std::{fmt, process::Command};

use serde::de::DeserializeOwned;

use crate::{command, display};

pub(crate) fn run_eval<T: DeserializeOwned>(command: Command) -> eyre::Result<Option<T>> {
    match command::output(command).and_then(|output| output.json::<T>()) {
        Ok(output) => Ok(Some(output)),
        Err(error) if error.is_exit_code_error() => {
            let stderr = String::from_utf8_lossy(error.stderr().unwrap_or(&[]));
            if stderr.is_empty() {
                tracing::error!("nix eval failed:\n  Captured stderr is empty");
            } else {
                tracing::error!(
                    "nix eval failed:\n  Catpured stderr:\n{}",
                    display::indent(4, &stderr),
                );
            }
            Ok(None)
        }
        Err(error) if error.is_json_error() => {
            tracing::error!(error = &error as &(dyn std::error::Error + Send + Sync));
            Ok(None)
        }
        Err(error) => Err(error.into_eyre()),
    }
}

/// Formats a given string as a string literal value in the Nix expression language.
///
/// See: <https://nix.dev/manual/nix/2.34/language/string-literals.html>
pub(crate) fn to_string_literal(s: &str) -> impl fmt::Display {
    use std::fmt::Write;

    fmt::from_fn(move |f| {
        let mut s = s;
        f.write_char('"')?;
        while !s.is_empty() {
            let mut next_escape_ix = s
                .find(['"', '\\', '$', '\n', '\r', '\t'])
                .unwrap_or(s.len());
            while s[next_escape_ix..].starts_with('$') && !s[next_escape_ix..].starts_with("${") {
                next_escape_ix = s[next_escape_ix + 1..]
                    .find(['"', '\\', '$', '\n', '\r', '\t'])
                    .map(|ix| ix + next_escape_ix + 1)
                    .unwrap_or(s.len());
            }

            f.write_str(&s[..next_escape_ix])?;
            s = &s[next_escape_ix..];

            if let Some(c) = s.chars().next() {
                f.write_char('\\')?;
                match c {
                    '"' | '\\' | '$' => f.write_char(c)?,
                    '\n' => f.write_char('n')?,
                    '\r' => f.write_char('r')?,
                    '\t' => f.write_char('t')?,
                    _ => unreachable!(),
                }
                s = &s[1..];
            }
        }
        f.write_char('"')?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn to_string_literal() {
        let cases = [
            // simple strings
            ("", r#""""#),
            ("abc", r#""abc""#),
            // `"` and `\`
            (r#"""#, r#""\"""#),
            (r#"\"#, r#""\\""#),
            (r#""" and "foo" and """#, r#""\"\" and \"foo\" and \"\"""#),
            (r#"\n \r \t"#, r#""\\n \\r \\t""#),
            // `$` and `${`
            ("$", r#""$""#),
            ("${", r#""\${""#),
            ("with $var", r#""with $var""#),
            ("like ${bash} :)", r#""like \${bash} :)""#),
            // `\n`, `\r`, `\t`
            ("\n", r#""\n""#),
            ("\r", r#""\r""#),
            ("\t", r#""\t""#),
            ("hello\nthre\rthere\tbye", r#""hello\nthre\rthere\tbye""#),
        ];

        for case in cases {
            let result = super::to_string_literal(case.0).to_string();
            assert_eq!(
                result, case.1,
                "{:?}: unexpected result of to_string_literal",
                case.0,
            );
        }
    }
}
