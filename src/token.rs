//! Lexer tokens and scanning.

use crate::error::{Error, ErrorKind, Span};

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Number(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    Bang,
    EqEq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    AndAnd,
    OrOr,
    Question,
    Colon,
    LParen,
    RParen,
    Comma,
    Dot,
    Eof,
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

pub fn tokenize(source: &str) -> Result<Vec<Token>, Error> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0usize;

    while i < source.len() {
        let start = i;
        let c = match source[i..].chars().next() {
            Some(ch) => ch,
            None => break,
        };

        if c.is_whitespace() {
            i += c.len_utf8();
            continue;
        }

        // Number: digit or leading dot with digit
        if c.is_ascii_digit() || (c == '.' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit())
        {
            let (value, end) = scan_number(source, i)?;
            tokens.push(Token {
                kind: TokenKind::Number(value),
                span: Span::new(start, end),
            });
            i = end;
            continue;
        }

        if is_ident_start(c) {
            let end = scan_ident(source, i);
            let ident = source[start..end].to_string();
            tokens.push(Token {
                kind: TokenKind::Ident(ident),
                span: Span::new(start, end),
            });
            i = end;
            continue;
        }

        let (kind, len) = match c {
            '+' => (TokenKind::Plus, 1),
            '-' => (TokenKind::Minus, 1),
            '*' => (TokenKind::Star, 1),
            '/' => (TokenKind::Slash, 1),
            '%' => (TokenKind::Percent, 1),
            '^' => (TokenKind::Caret, 1),
            '(' => (TokenKind::LParen, 1),
            ')' => (TokenKind::RParen, 1),
            ',' => (TokenKind::Comma, 1),
            '.' => (TokenKind::Dot, 1),
            '?' => (TokenKind::Question, 1),
            ':' => (TokenKind::Colon, 1),
            '!' if peek(bytes, i + 1) == Some(b'=') => (TokenKind::NotEq, 2),
            '!' => (TokenKind::Bang, 1),
            '=' if peek(bytes, i + 1) == Some(b'=') => (TokenKind::EqEq, 2),
            '<' if peek(bytes, i + 1) == Some(b'=') => (TokenKind::LtEq, 2),
            '<' => (TokenKind::Lt, 1),
            '>' if peek(bytes, i + 1) == Some(b'=') => (TokenKind::GtEq, 2),
            '>' => (TokenKind::Gt, 1),
            '&' if peek(bytes, i + 1) == Some(b'&') => (TokenKind::AndAnd, 2),
            '|' if peek(bytes, i + 1) == Some(b'|') => (TokenKind::OrOr, 2),
            _ => {
                return Err(
                    Error::new(ErrorKind::Lex, format!("unexpected character `{c}`"))
                        .with_span(Span::new(start, start + c.len_utf8())),
                );
            }
        };

        tokens.push(Token {
            kind,
            span: Span::new(start, start + len),
        });
        i += len;
    }

    tokens.push(Token {
        kind: TokenKind::Eof,
        span: Span::new(source.len(), source.len()),
    });
    Ok(tokens)
}

fn peek(bytes: &[u8], i: usize) -> Option<u8> {
    bytes.get(i).copied()
}

fn scan_ident(source: &str, start: usize) -> usize {
    let mut end = start;
    for (off, ch) in source[start..].char_indices() {
        if off == 0 {
            if !is_ident_start(ch) {
                break;
            }
            end = start + ch.len_utf8();
            continue;
        }
        if is_ident_continue(ch) {
            end = start + off + ch.len_utf8();
        } else {
            break;
        }
    }
    end
}

fn scan_number(source: &str, start: usize) -> Result<(f64, usize), Error> {
    let bytes = source.as_bytes();
    let mut i = start;
    let mut seen_dot = false;
    let mut seen_exp = false;

    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_ascii_digit() {
            i += 1;
        } else if c == '.' && !seen_dot && !seen_exp {
            seen_dot = true;
            i += 1;
        } else if (c == 'e' || c == 'E') && !seen_exp {
            seen_exp = true;
            i += 1;
            if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
                i += 1;
            }
            if i >= bytes.len() || !bytes[i].is_ascii_digit() {
                return Err(Error::new(ErrorKind::Lex, "malformed exponent in number")
                    .with_span(Span::new(start, i.min(source.len()))));
            }
        } else {
            break;
        }
    }

    let text = &source[start..i];
    let value: f64 = text.parse().map_err(|_| {
        Error::new(ErrorKind::Lex, format!("invalid number `{text}`"))
            .with_span(Span::new(start, i))
    })?;
    Ok((value, i))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_idents() {
        let toks = tokenize("foo.bar 1.5e-2").unwrap();
        assert!(matches!(toks[0].kind, TokenKind::Ident(ref s) if s == "foo"));
        assert!(matches!(toks[1].kind, TokenKind::Dot));
        assert!(matches!(toks[2].kind, TokenKind::Ident(ref s) if s == "bar"));
        assert!(matches!(toks[3].kind, TokenKind::Number(n) if (n - 0.015).abs() < 1e-12));
    }

    #[test]
    fn unicode_idents() {
        let toks = tokenize("等级*10+200").unwrap();
        assert!(matches!(toks[0].kind, TokenKind::Ident(ref s) if s == "等级"));
        assert_eq!(toks[1].kind, TokenKind::Star);
        assert!(matches!(toks[2].kind, TokenKind::Number(n) if (n - 10.0).abs() < 1e-12));
        assert_eq!(toks[3].kind, TokenKind::Plus);
        assert!(matches!(toks[4].kind, TokenKind::Number(n) if (n - 200.0).abs() < 1e-12));
    }

    #[test]
    fn multi_char_ops() {
        let toks = tokenize("a && b || c != d == e <= f >= g").unwrap();
        assert!(toks.iter().any(|t| t.kind == TokenKind::AndAnd));
        assert!(toks.iter().any(|t| t.kind == TokenKind::OrOr));
        assert!(toks.iter().any(|t| t.kind == TokenKind::NotEq));
        assert!(toks.iter().any(|t| t.kind == TokenKind::EqEq));
    }
}
