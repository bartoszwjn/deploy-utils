//! Natural sorting of strings.

use std::{borrow::Cow, cmp::Ordering};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NaturalString<'a>(Cow<'a, str>);

impl<'a> NaturalString<'a> {
    pub(crate) fn owned(s: String) -> Self {
        NaturalString(Cow::Owned(s))
    }

    pub(crate) fn borrowed(s: &'a str) -> Self {
        NaturalString(Cow::Borrowed(s))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl PartialOrd for NaturalString<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for NaturalString<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        split(&self.0).cmp(split(&other.0))
    }
}

fn split(mut s: &str) -> impl Iterator<Item = Component<'_>> {
    std::iter::from_fn(move || {
        let is_number = match s.chars().next() {
            None => return None,
            Some(c) => c.is_ascii_digit(),
        };

        let chunk_end = s
            .find(|c: char| c.is_ascii_digit() != is_number)
            .unwrap_or(s.len());
        let (chunk, rest) = s.split_at(chunk_end);

        s = rest;
        Some(if is_number {
            Component::Number(chunk)
        } else {
            Component::Text(chunk)
        })
    })
}

#[derive(Debug, Eq, PartialEq)]
enum Component<'a> {
    Text(&'a str),
    Number(&'a str),
}

impl PartialOrd for Component<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Component<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Component::Text(l), Component::Text(r)) => l.cmp(r),
            (Component::Text(_), Component::Number(_)) => Ordering::Greater,
            (Component::Number(_), Component::Text(_)) => Ordering::Less,
            (Component::Number(l), Component::Number(r)) => {
                fn split_leading_zeros(s: &str) -> (&str, &str) {
                    s.split_at(s.find(|c| c != '0').unwrap_or(s.len()))
                }

                let (zeros_l, val_l) = split_leading_zeros(l);
                let (zeros_r, val_r) = split_leading_zeros(r);

                (val_l.len().cmp(&val_r.len()))
                    .then_with(|| val_l.cmp(val_r))
                    .then_with(|| zeros_l.len().cmp(&zeros_r.len()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::NaturalString;

    #[test]
    fn ord() {
        use std::cmp::Ordering::{Equal, Greater, Less};

        for (l, r, exp) in [
            // equal values
            ("", "", Equal),
            ("a", "a", Equal),
            ("abc", "abc", Equal),
            ("0", "0", Equal),
            ("1", "1", Equal),
            ("123", "123", Equal),
            ("ab12", "ab12", Equal),
            ("1a2b3c", "1a2b3c", Equal),
            ("abc1234def", "abc1234def", Equal),
            ("0042", "0042", Equal),
            // empty string
            ("", "a", Less),
            ("", "1", Less),
            ("", "abc123", Less),
            ("", "123abc", Less),
            // text
            ("a", "b", Less),
            ("abc", "abd", Less),
            ("abd", "acd", Less),
            ("aa", "a", Greater),
            // numbers
            ("0", "1", Less),
            ("10", "9", Greater),
            ("6", "7", Less),
            ("123", "531", Less),
            ("534", "1234", Less),
            // text and numbers mixed
            ("a 1 b 2 c 3", "a 1 b 2 c 4", Less),
            ("a 1 b 3 c 4", "a 1 b 2 c 4", Greater),
            ("a 1 b 2 c 3", "a 1 b 2 d 3", Less),
            ("a 1 c 2 d 3", "a 1 b 2 d 3", Greater),
            // text vs numbers
            ("a", "1", Greater),
            ("a 1 b 2 c 3", "a 1 b 2 cd", Less),
            ("a 1 b 2 c 3", "a 1 b 22 c 3", Less),
            // leading zeros
            ("7", "07", Less),
            ("0042", "000042", Less),
            ("abc0042", "abc000042", Less),
        ] {
            let l = NaturalString::borrowed(l);
            let r = NaturalString::borrowed(r);

            assert_eq!(l.cmp(&r), exp, "{l:?}.cmp(&{r:?}) != {exp:?}");

            assert_eq!(
                r.cmp(&l),
                exp.reverse(),
                "Ord::cmp is not symmetric: {r:?}.cmp(&{l:?}) != {:?}",
                exp.reverse(),
            );

            if exp == Equal {
                assert_eq!(l, r, "Ord::cmp result does not match Eq::eq");
            } else {
                assert_ne!(l, r, "Ord::cmp result does not match Eq::eq");
            }
        }
    }
}
