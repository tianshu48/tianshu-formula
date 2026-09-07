//! Pratt / precedence-climbing parser.

use crate::ast::{BinaryOp, Expr, UnaryOp};

/// Pratt descent uses many frames per paren/unary nest; keep this well below
/// the native stack limit. Flat add-chains are capped later in `validate_calls`.
const MAX_PARSE_NESTING: usize = 64;
use crate::error::{Error, ErrorKind, Span};
use crate::token::{tokenize, Token, TokenKind};

pub fn parse(source: &str) -> Result<Expr, Error> {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return Err(Error::new(ErrorKind::EmptyInput, "empty formula"));
    }
    let tokens = tokenize(source)?;
    let mut parser = Parser {
        tokens: &tokens,
        pos: 0,
        depth: 0,
    };
    let expr = parser.parse_ternary()?;
    if !parser.is_eof() {
        let t = parser.peek();
        return Err(Error::new(
            ErrorKind::Parse,
            format!("unexpected token at end of input: {:?}", t.kind),
        )
        .with_span(t.span));
    }
    Ok(expr)
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    depth: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn is_eof(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Eof)
    }

    fn descend(&mut self) -> Result<(), Error> {
        self.depth += 1;
        if self.depth > MAX_PARSE_NESTING {
            self.depth -= 1;
            return Err(Error::new(ErrorKind::Parse, "formula too deeply nested"));
        }
        Ok(())
    }

    fn ascend(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    fn bump(&mut self) -> &Token {
        let t = &self.tokens[self.pos];
        if !matches!(t.kind, TokenKind::Eof) {
            self.pos += 1;
        }
        t
    }

    fn parse_ternary(&mut self) -> Result<Expr, Error> {
        let cond = self.parse_or()?;
        if matches!(self.peek().kind, TokenKind::Question) {
            let q_span = self.bump().span;
            self.descend()?;
            let then_branch = self.parse_ternary();
            self.ascend();
            let then_branch = then_branch?;
            if !matches!(self.peek().kind, TokenKind::Colon) {
                return Err(Error::new(ErrorKind::Parse, "expected `:` in ternary")
                    .with_span(self.peek().span));
            }
            self.bump();
            self.descend()?;
            let else_branch = self.parse_ternary();
            self.ascend();
            let else_branch = else_branch?;
            let span = Span::new(cond.span().start, else_branch.span().end.max(q_span.end));
            return Ok(Expr::Ternary {
                cond: Box::new(cond),
                then_branch: Box::new(then_branch),
                else_branch: Box::new(else_branch),
                span,
            });
        }
        Ok(cond)
    }

    fn parse_or(&mut self) -> Result<Expr, Error> {
        let mut left = self.parse_and()?;
        while matches!(self.peek().kind, TokenKind::OrOr) {
            self.bump();
            let right = self.parse_and()?;
            let span = Span::new(left.span().start, right.span().end);
            left = Expr::Binary {
                op: BinaryOp::Or,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, Error> {
        let mut left = self.parse_cmp()?;
        while matches!(self.peek().kind, TokenKind::AndAnd) {
            self.bump();
            let right = self.parse_cmp()?;
            let span = Span::new(left.span().start, right.span().end);
            left = Expr::Binary {
                op: BinaryOp::And,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_cmp(&mut self) -> Result<Expr, Error> {
        let mut left = self.parse_add()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::Lt => BinaryOp::Lt,
                TokenKind::LtEq => BinaryOp::Le,
                TokenKind::Gt => BinaryOp::Gt,
                TokenKind::GtEq => BinaryOp::Ge,
                TokenKind::EqEq => BinaryOp::Eq,
                TokenKind::NotEq => BinaryOp::Ne,
                _ => break,
            };
            self.bump();
            let right = self.parse_add()?;
            let span = Span::new(left.span().start, right.span().end);
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_add(&mut self) -> Result<Expr, Error> {
        let mut left = self.parse_mul()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::Plus => BinaryOp::Add,
                TokenKind::Minus => BinaryOp::Sub,
                _ => break,
            };
            self.bump();
            let right = self.parse_mul()?;
            let span = Span::new(left.span().start, right.span().end);
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_mul(&mut self) -> Result<Expr, Error> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::Star => BinaryOp::Mul,
                TokenKind::Slash => BinaryOp::Div,
                TokenKind::Percent => BinaryOp::Mod,
                _ => break,
            };
            self.bump();
            let right = self.parse_unary()?;
            let span = Span::new(left.span().start, right.span().end);
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, Error> {
        if matches!(
            self.peek().kind,
            TokenKind::Plus | TokenKind::Minus | TokenKind::Bang
        ) {
            self.descend()?;
            let result = self.parse_unary_op();
            self.ascend();
            return result;
        }
        self.parse_pow()
    }

    fn parse_unary_op(&mut self) -> Result<Expr, Error> {
        match self.peek().kind {
            TokenKind::Plus => {
                let start = self.bump().span.start;
                let expr = self.parse_unary()?;
                let span = Span::new(start, expr.span().end);
                Ok(Expr::Unary {
                    op: UnaryOp::Plus,
                    expr: Box::new(expr),
                    span,
                })
            }
            TokenKind::Minus => {
                let start = self.bump().span.start;
                let expr = self.parse_unary()?;
                let span = Span::new(start, expr.span().end);
                Ok(Expr::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(expr),
                    span,
                })
            }
            TokenKind::Bang => {
                let start = self.bump().span.start;
                let expr = self.parse_unary()?;
                let span = Span::new(start, expr.span().end);
                Ok(Expr::Unary {
                    op: UnaryOp::Not,
                    expr: Box::new(expr),
                    span,
                })
            }
            _ => self.parse_pow(),
        }
    }

    fn parse_pow(&mut self) -> Result<Expr, Error> {
        let left = self.parse_primary()?;
        if matches!(self.peek().kind, TokenKind::Caret) {
            self.bump();
            let right = self.parse_pow_rhs()?;
            let span = Span::new(left.span().start, right.span().end);
            return Ok(Expr::Binary {
                op: BinaryOp::Pow,
                left: Box::new(left),
                right: Box::new(right),
                span,
            });
        }
        Ok(left)
    }

    fn parse_pow_rhs(&mut self) -> Result<Expr, Error> {
        // RHS of ^: unary (incl. nested power via parse_pow at end of unary chain)
        match self.peek().kind {
            TokenKind::Plus | TokenKind::Minus | TokenKind::Bang => self.parse_unary(),
            _ => {
                let left = self.parse_primary()?;
                if matches!(self.peek().kind, TokenKind::Caret) {
                    self.bump();
                    let right = self.parse_pow_rhs()?;
                    let span = Span::new(left.span().start, right.span().end);
                    Ok(Expr::Binary {
                        op: BinaryOp::Pow,
                        left: Box::new(left),
                        right: Box::new(right),
                        span,
                    })
                } else {
                    Ok(left)
                }
            }
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, Error> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::Number(value) => {
                self.bump();
                Ok(Expr::Literal {
                    value,
                    span: token.span,
                })
            }
            TokenKind::Ident(_) => self.parse_ident_or_call(),
            TokenKind::LParen => {
                self.descend()?;
                self.bump();
                let expr = self.parse_ternary();
                self.ascend();
                let expr = expr?;
                if !matches!(self.peek().kind, TokenKind::RParen) {
                    return Err(
                        Error::new(ErrorKind::Parse, "expected `)`").with_span(self.peek().span)
                    );
                }
                let end = self.bump().span.end;
                // Keep inner span; optionally widen — keep inner for clarity
                let _ = (token.span.start, end);
                Ok(expr)
            }
            TokenKind::Eof => {
                Err(Error::new(ErrorKind::Parse, "unexpected end of input").with_span(token.span))
            }
            _ => Err(Error::new(
                ErrorKind::Parse,
                format!("unexpected token: {:?}", token.kind),
            )
            .with_span(token.span)),
        }
    }

    fn parse_ident_or_call(&mut self) -> Result<Expr, Error> {
        let first = self.bump().clone();
        let TokenKind::Ident(mut name) = first.kind else {
            return Err(Error::new(ErrorKind::Parse, "expected identifier").with_span(first.span));
        };
        let mut span = first.span;

        // Dotted path: foo.bar.baz as single variable key (unless followed by '(')
        while matches!(self.peek().kind, TokenKind::Dot) {
            self.bump();
            let next = self.peek().clone();
            let TokenKind::Ident(part) = next.kind else {
                return Err(
                    Error::new(ErrorKind::Parse, "expected identifier after `.`")
                        .with_span(next.span),
                );
            };
            self.bump();
            name.push('.');
            name.push_str(&part);
            span = Span::new(span.start, next.span.end);
        }

        if matches!(self.peek().kind, TokenKind::LParen) {
            // Function call — name must be a simple ident historically; allow dotted? Plan says name(args).
            // Dotted function names are unusual; allow only if we treated whole thing as name.
            self.bump();
            let mut args = Vec::new();
            if !matches!(self.peek().kind, TokenKind::RParen) {
                loop {
                    self.descend()?;
                    let arg = self.parse_ternary();
                    self.ascend();
                    args.push(arg?);
                    if matches!(self.peek().kind, TokenKind::Comma) {
                        self.bump();
                        continue;
                    }
                    break;
                }
            }
            if !matches!(self.peek().kind, TokenKind::RParen) {
                return Err(Error::new(ErrorKind::Parse, "expected `)` after arguments")
                    .with_span(self.peek().span));
            }
            let end = self.bump().span.end;
            span = Span::new(span.start, end);
            return Ok(Expr::Call { name, args, span });
        }

        Ok(Expr::Variable { name, span })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precedence_mul_add() {
        let e = parse("1 + 2 * 3").unwrap();
        match e {
            Expr::Binary {
                op: BinaryOp::Add,
                right,
                ..
            } => match *right {
                Expr::Binary {
                    op: BinaryOp::Mul, ..
                } => {}
                other => panic!("expected mul, got {other:?}"),
            },
            other => panic!("expected add, got {other:?}"),
        }
    }

    #[test]
    fn pow_right_assoc() {
        let e = parse("2^3^2").unwrap();
        match e {
            Expr::Binary {
                op: BinaryOp::Pow,
                right,
                ..
            } => match *right {
                Expr::Binary {
                    op: BinaryOp::Pow, ..
                } => {}
                other => panic!("expected nested pow, got {other:?}"),
            },
            other => panic!("expected pow, got {other:?}"),
        }
    }

    #[test]
    fn ternary_and_call() {
        let e = parse("a > 0 ? max(a, b) : 0").unwrap();
        assert!(matches!(e, Expr::Ternary { .. }));
    }

    #[test]
    fn dotted_variable() {
        let e = parse("foo.bar").unwrap();
        match e {
            Expr::Variable { name, .. } => assert_eq!(name, "foo.bar"),
            other => panic!("expected variable, got {other:?}"),
        }
    }
}
