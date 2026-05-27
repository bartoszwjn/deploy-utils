//! Deploy targets.

use std::{fmt, str::FromStr};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Target {
    node: String,
    profile: Option<String>,
}

impl Target {
    pub(crate) fn node(&self) -> &str {
        &self.node
    }

    pub(crate) fn profile(&self) -> Option<&str> {
        self.profile.as_deref()
    }

    fn from_parts(parts: Vec<String>) -> Result<Self, ParseError> {
        let mut parts = parts.into_iter().fuse();
        let Some(node) = parts.next() else {
            return Err(ParseError::EmptyPath);
        };
        let profile = parts.next();
        if parts.next().is_some() {
            return Err(ParseError::TooManyComponents);
        }
        Ok(Self { node, profile })
    }
}

impl FromStr for Target {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // References:
        // https://git.lix.systems/lix-project/lix/src/commit/7831c98a4db589c84cf730db23793afe3fd90f2d/lix/libexpr/attr-path.hh#L23
        // https://git.lix.systems/lix-project/lix/src/commit/7831c98a4db589c84cf730db23793afe3fd90f2d/lix/libexpr/attr-path.cc#L10

        if s.starts_with(".") {
            return Err(ParseError::LeadingDot);
        }

        if s.is_empty() {
            return Self::from_parts(vec![]);
        }

        let mut parts = Vec::new();
        let mut current = String::new();
        let mut started = false;
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            match c {
                '.' => {
                    if !started {
                        // This cannot be a leading dot, since we checked for that at the start.
                        return Err(ParseError::ConsecutiveDots);
                    }
                    parts.push(std::mem::take(&mut current));
                    started = false;
                }
                '"' => {
                    started = true;
                    loop {
                        match chars.next() {
                            None => return Err(ParseError::NoClosingQuote),
                            Some('"') => break,
                            Some(c) => current.push(c),
                        }
                    }
                }
                c => {
                    started = true;
                    current.push(c);
                }
            }
        }
        if started {
            parts.push(current);
        } else {
            return Err(ParseError::TrailingDot);
        }

        Self::from_parts(parts)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ParseError {
    ConsecutiveDots,
    LeadingDot,
    TrailingDot,
    NoClosingQuote,
    EmptyPath,
    TooManyComponents,
}

impl std::error::Error for ParseError {}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl ParseError {
    fn message(&self) -> &'static str {
        match self {
            ParseError::ConsecutiveDots => {
                "consecutive dots are not allowed in attribute paths \
                (empty attribute names must be quoted)"
            }
            ParseError::LeadingDot => {
                "leading dots are not allowed in attribute paths \
                (empty attribute names must be quoted)"
            }
            ParseError::TrailingDot => {
                "trailing dots are not allowed in attribute paths \
                (empty attribute names must be quoted)"
            }
            ParseError::NoClosingQuote => "missing closing quote in attribute path",
            ParseError::EmptyPath => {
                "attribute paths must contain at least one component \
                (empty attribute names must be quoted)"
            }
            ParseError::TooManyComponents => {
                "attribute paths can contain at most 2 components \
                (attribute names containing dots must be quoted)"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::{ParseError, Target};

    #[test]
    fn parse_cli_arg() {
        let cases: [(&str, (&str, Option<&str>)); _] = [
            // simple
            ("a", ("a", None)),
            ("foo.bar", ("foo", Some("bar"))),
            // quoting
            (r#""foo""#, ("foo", None)),
            (r#"f"o"o"#, ("foo", None)),
            (r#""foo.bar""#, ("foo.bar", None)),
            (r#"foo."bar.baz""#, ("foo", Some("bar.baz"))),
            (r#"foo.bar"."baz"#, ("foo", Some("bar.baz"))),
            (r#"".foo"."bar.""#, (".foo", Some("bar."))),
            // empty
            (r#""""#, ("", None)),
            (r#"""."""#, ("", Some(""))),
        ];

        for (input, expected) in cases {
            let parsed = Target::from_str(input);
            let expected = Target {
                node: expected.0.to_owned(),
                profile: expected.1.map(str::to_owned),
            };
            assert_eq!(
                Ok(expected),
                parsed,
                "{input:?}: unexpected result of Target::from_str"
            );
        }
    }

    #[test]
    fn parse_cli_arg_errors() {
        let cases: [(&str, ParseError); _] = [
            (".", ParseError::LeadingDot),
            (".foo", ParseError::LeadingDot),
            ("..", ParseError::LeadingDot),
            ("foo.", ParseError::TrailingDot),
            ("foo.bar.", ParseError::TrailingDot),
            ("foo..", ParseError::ConsecutiveDots),
            ("foo..bar", ParseError::ConsecutiveDots),
            ("foo.bar..", ParseError::ConsecutiveDots),
            (r#"""#, ParseError::NoClosingQuote),
            (r#"foo."bar"#, ParseError::NoClosingQuote),
            (r#""foo".bar""#, ParseError::NoClosingQuote),
            ("", ParseError::EmptyPath),
            ("foo.bar.baz", ParseError::TooManyComponents),
            ("foo.bar.baz.quux", ParseError::TooManyComponents),
        ];

        for (input, expected) in cases {
            let result = Target::from_str(input);
            assert_eq!(
                Err(expected),
                result,
                "{input:?}: unexpected result of Target::from_str"
            );
        }
    }
}
