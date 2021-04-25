use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref PUB_MOD_REGEX: Regex = Regex::new(r"^\s*(pub)?\s*mod (.+);").unwrap();
    static ref USE_SINGLE_REGEX: Regex = Regex::new(r"^\s*use\s*((\w+::)*([\w]+));").unwrap();
    static ref USE_MULTI_REGEX: Regex = Regex::new(r"^\s*use\s*(((\w+)::)+\{(.+)\};)").unwrap();
    static ref OTHER_LINE_REGEX: Regex = Regex::new(r"^\s*(?P<line>.+?)\s*$").unwrap();
}

#[derive(Eq, PartialEq, Debug)]
pub enum LineToken {
    DeclareOtherModule {
        line: String,
        name: LineRef,
        is_pub: bool,
    },
    UseModule {
        line: String,
        name: LineRef,
    },
    UseManyModules {
        names: Vec<LineRef>,
        line: String,
        parent: LineRef,
    },
    Module {
        name: String,
        is_pub: bool,
        tokens: Vec<LineToken>,
    },
    OtherLine {
        line: String,
        trimmed_ref: LineRef,
    },
}

#[derive(Eq, PartialEq, Debug)]
pub struct LineRef {
    start: usize,
    size: usize,
}

impl LineRef {
    pub fn new(start: usize, size: usize) -> Self {
        Self { start, size }
    }

    fn from_match(m: regex::Match) -> Self {
        Self {
            start: m.start(),
            size: m.as_str().len(),
        }
    }

    pub fn resolve_unchecked<'a>(&self, line: &'a str) -> &'a str {
        &line[self.start..self.start + self.size]
    }
}

struct LineRefTokenizer<'a> {
    line: &'a str,
    start: usize,
    size: usize,
}

impl<'a> LineRefTokenizer<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            line: input,
            start: 0,
            size: 0,
        }
    }

    fn yield_line_ref(&mut self) -> LineRef {
        let result = LineRef::new(self.start, self.size);
        self.start = self.start + self.size + 1;
        self.size = 0;
        result
    }
}

impl<'a> Iterator for LineRefTokenizer<'a> {
    type Item = LineRef;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let next = self.line.chars().nth(self.start + self.size);

            match next {
                Some(c) if c == ',' => {
                    return Some(self.yield_line_ref());
                }
                Some(c) if c == ' ' && self.size == 0 => {
                    self.start += 1;
                }

                Some(_) => {
                    self.size += 1;
                }

                None if self.size > 0 => {
                    return Some(self.yield_line_ref());
                }
                None if self.size == 0 => {
                    return None;
                }
                _ => return None,
            }
        }
    }
}

pub fn parse_line<'a>(line: String) -> LineToken {
    if let Some(c) = PUB_MOD_REGEX.captures(&line) {
        let is_pub = c.get(1).is_some();
        let line_ref = LineRef::from_match(c.get(2).unwrap());
        return LineToken::DeclareOtherModule {
            line: line,
            name: line_ref,
            is_pub: is_pub,
        };
    }

    if let Some(c) = USE_SINGLE_REGEX.captures(&line) {
        let line_ref = LineRef::from_match(c.get(1).unwrap());
        return LineToken::UseModule {
            line: line,
            name: line_ref,
        };
    }

    if let Some(c) = USE_MULTI_REGEX.captures(&line) {
        let line_ref = LineRef::from_match(c.get(4).unwrap());
        let names = LineRefTokenizer::new(line_ref.resolve_unchecked(&line))
            .map(|mut r| {
                r.start += line_ref.start;
                r
            })
            .collect::<Vec<LineRef>>();
        let parent = LineRef::from_match(c.get(3).unwrap());
        return LineToken::UseManyModules {
            line: line,
            names: names,
            parent: parent,
        };
    }
    if let Some(c) = OTHER_LINE_REGEX.captures(&line) {
        let line_ref = LineRef::from_match(c.name("line").unwrap());
        return LineToken::OtherLine {
            line: line,
            trimmed_ref: line_ref,
        };
    }
    LineToken::OtherLine {
        trimmed_ref: LineRef::new(0, line.len()),
        line: line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn line_ref_resolve_works() {
        let line = "Hello, world";
        let line_ref = LineRef::new(0, 5);
        assert_eq!("Hello", line_ref.resolve_unchecked(line));
    }

    #[test]
    fn line_ref_empty_works() {
        let line = "test";
        let line_ref = LineRef::new(0, 0);
        assert_eq!("", line_ref.resolve_unchecked(line));
    }

    #[test]
    fn line_ref_tokenizer_works() {
        let line = "a, bb, cccc";
        let tokens: Vec<LineRef> = LineRefTokenizer::new(line).collect();
        assert_eq!(
            vec![LineRef::new(0, 1), LineRef::new(3, 2), LineRef::new(7, 4)],
            tokens
        );
    }

    #[test]
    fn line_ref_tokenizer_spaces() {
        let line = "a,  bb,cccc";
        let tokens: Vec<LineRef> = LineRefTokenizer::new(line).collect();
        assert_eq!(
            vec![LineRef::new(0, 1), LineRef::new(4, 2), LineRef::new(7, 4)],
            tokens
        );
    }

    #[test]
    fn parse_line_pub_mod() {
        let line = "pub mod game;".to_string();
        let token = parse_line(line);
        let expected = LineToken::DeclareOtherModule {
            line: "pub mod game;".to_string(),
            name: LineRef::new(8, 4),
            is_pub: true,
        };
        assert_eq!(token, expected);
    }

    #[test]
    fn parse_line_nonpub_mod() {
        let line = "mod test_this;".to_string();
        let token = parse_line(line.clone());
        let expected = LineToken::DeclareOtherModule {
            line: line,
            name: LineRef::new(4, 9),
            is_pub: false,
        };
        assert_eq!(token, expected);
    }

    #[test]
    fn parse_line_use_mod() {
        let line = "use std::io::Buf;".to_string();
        let token = parse_line(line.clone());
        let expected = LineToken::UseModule {
            line: line,
            name: LineRef::new(4, 12),
        };
        assert_eq!(token, expected);
    }

    #[test]
    fn parse_line_use_multy_mod() {
        let line = "use std::{collections::HashSet, io::BufWriter};".to_string();
        let token = parse_line(line.clone());
        let expected = LineToken::UseManyModules {
            line: line,
            names: vec![LineRef::new(10, 20), LineRef::new(32, 13)],
            parent: LineRef::new(4, 3),
        };
        assert_eq!(token, expected);
    }

    #[test]
    fn parse_line_other_line() {
        let line = "   class Turn  ".to_string();
        let token = parse_line(line.clone());
        let expected = LineToken::OtherLine {
            line: line.clone(),
            trimmed_ref: LineRef::new(3, line.trim().len()),
        };
        assert_eq!(token, expected);
    }
}
