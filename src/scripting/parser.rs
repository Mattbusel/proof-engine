//! Recursive-descent parser — converts token stream into AST.

use super::lexer::{Token, TokenWithSpan};
use super::ast::*;
use std::fmt;

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub line:    u32,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Parse error at line {}: {}", self.line, self.message)
    }
}

pub struct Parser {
    tokens:  Vec<TokenWithSpan>,
    pos:     usize,
}

impl Parser {
    pub fn new(tokens: Vec<TokenWithSpan>) -> Self {
        Self { tokens, pos: 0 }
    }

    /// Convenience: lex + parse from source in one call.
    pub fn from_source(name: &str, source: &str) -> Result<Script, ParseError> {
        let mut lexer = super::lexer::Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens);
        parser.parse_script(name)
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).map(|t| &t.token).unwrap_or(&Token::Eof)
    }

    fn peek_span_line(&self) -> u32 {
        self.tokens.get(self.pos).map(|t| t.span.line).unwrap_or(0)
    }

    fn advance(&mut self) -> &Token {
        let t = self.tokens.get(self.pos).map(|t| &t.token).unwrap_or(&Token::Eof);
        if self.pos < self.tokens.len() { self.pos += 1; }
        t
    }

    fn expect(&mut self, expected: &Token) -> Result<(), ParseError> {
        if self.peek() == expected {
            self.advance();
            Ok(())
        } else {
            Err(ParseError {
                message: format!("expected {:?}, got {:?}", expected, self.peek()),
                line: self.peek_span_line(),
            })
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        match self.peek().clone() {
            Token::Ident(s) => { self.advance(); Ok(s) }
            t => Err(ParseError { message: format!("expected identifier, got {:?}", t), line: self.peek_span_line() })
        }
    }

    fn check(&self, t: &Token) -> bool { self.peek() == t }

    fn consume_if(&mut self, t: &Token) -> bool {
        if self.peek() == t { self.advance(); true }
        else { false }
    }

    // ── Statement parsing ─────────────────────────────────────────────────

    pub fn parse_script(&mut self, name: &str) -> Result<Script, ParseError> {
        let stmts = self.parse_block()?;
        Ok(Script { name: name.to_string(), stmts })
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut stmts = Vec::new();
        loop {
            self.consume_if(&Token::Semicolon);
            match self.peek() {
                Token::Eof | Token::End | Token::Else | Token::ElseIf | Token::Until => break,
                _ => {}
            }
            stmts.push(self.parse_stmt()?);
        }
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        match self.peek().clone() {
            Token::Local    => self.parse_local(),
            Token::Function => self.parse_func_decl(),
            Token::If       => self.parse_if(),
            Token::While    => self.parse_while(),
            Token::For      => self.parse_for(),
            Token::Repeat   => self.parse_repeat_until(),
            Token::Do       => { self.advance(); let b = self.parse_block()?; self.expect(&Token::End)?; Ok(Stmt::Do(b)) }
            Token::Return   => self.parse_return(),
            Token::Break    => { self.advance(); Ok(Stmt::Break) }
            Token::Continue => { self.advance(); Ok(Stmt::Continue) }
            Token::Match    => self.parse_match(),
            Token::Import   => self.parse_import(),
            Token::Export   => { self.advance(); let name = self.expect_ident()?; Ok(Stmt::Export(name)) }
            _               => self.parse_expr_stmt(),
        }
    }

    fn parse_local(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume 'local'
        if self.check(&Token::Function) {
            self.advance();
            let name = self.expect_ident()?;
            let (params, vararg, body) = self.parse_func_body()?;
            return Ok(Stmt::LocalFunc { name, params, vararg, body });
        }
        let mut names = vec![self.expect_ident()?];
        while self.consume_if(&Token::Comma) {
            names.push(self.expect_ident()?);
        }
        let inits = if self.consume_if(&Token::Eq) {
            self.parse_expr_list()?
        } else { Vec::new() };

        if names.len() == 1 && inits.len() <= 1 {
            Ok(Stmt::LocalDecl { name: names.remove(0), init: inits.into_iter().next() })
        } else {
            Ok(Stmt::LocalMulti { names, inits })
        }
    }

    fn parse_func_decl(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume 'function'
        let mut name = vec![self.expect_ident()?];
        while self.consume_if(&Token::Dot) {
            name.push(self.expect_ident()?);
        }
        let (params, vararg, body) = self.parse_func_body()?;
        Ok(Stmt::FuncDecl { name, params, vararg, body })
    }

    fn parse_func_body(&mut self) -> Result<(Vec<String>, bool, Vec<Stmt>), ParseError> {
        self.expect(&Token::LParen)?;
        let mut params = Vec::new();
        let mut vararg = false;
        if !self.check(&Token::RParen) {
            loop {
                if self.check(&Token::DotDotDot) {
                    self.advance();
                    vararg = true;
                    break;
                }
                params.push(self.expect_ident()?);
                if !self.consume_if(&Token::Comma) { break; }
            }
        }
        self.expect(&Token::RParen)?;
        let body = self.parse_block()?;
        self.expect(&Token::End)?;
        Ok((params, vararg, body))
    }

    fn parse_if(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // if
        let cond = self.parse_expr(0)?;
        self.expect(&Token::Then)?;
        let then_body = self.parse_block()?;
        let mut elseif_branches = Vec::new();
        let mut else_body = None;
        loop {
            if self.consume_if(&Token::ElseIf) {
                let ec = self.parse_expr(0)?;
                self.expect(&Token::Then)?;
                let eb = self.parse_block()?;
                elseif_branches.push((ec, eb));
            } else if self.consume_if(&Token::Else) {
                else_body = Some(self.parse_block()?);
                break;
            } else { break; }
        }
        self.expect(&Token::End)?;
        Ok(Stmt::If { cond, then_body, elseif_branches, else_body })
    }

    fn parse_while(&mut self) -> Result<Stmt, ParseError> {
        self.advance();
        let cond = self.parse_expr(0)?;
        self.expect(&Token::Do)?;
        let body = self.parse_block()?;
        self.expect(&Token::End)?;
        Ok(Stmt::While { cond, body })
    }

    fn parse_for(&mut self) -> Result<Stmt, ParseError> {
        self.advance();
        let first = self.expect_ident()?;
        if self.consume_if(&Token::Eq) {
            // numeric for
            let start = self.parse_expr(0)?;
            self.expect(&Token::Comma)?;
            let limit = self.parse_expr(0)?;
            let step = if self.consume_if(&Token::Comma) { Some(self.parse_expr(0)?) } else { None };
            self.expect(&Token::Do)?;
            let body = self.parse_block()?;
            self.expect(&Token::End)?;
            Ok(Stmt::NumericFor { var: first, start, limit, step, body })
        } else {
            // generic for
            let mut vars = vec![first];
            while self.consume_if(&Token::Comma) { vars.push(self.expect_ident()?); }
            self.expect(&Token::In)?;
            let iter = self.parse_expr_list()?;
            self.expect(&Token::Do)?;
            let body = self.parse_block()?;
            self.expect(&Token::End)?;
            Ok(Stmt::GenericFor { vars, iter, body })
        }
    }

    fn parse_repeat_until(&mut self) -> Result<Stmt, ParseError> {
        self.advance();
        let body = self.parse_block()?;
        self.expect(&Token::Until)?;
        let cond = self.parse_expr(0)?;
        Ok(Stmt::RepeatUntil { body, cond })
    }

    fn parse_return(&mut self) -> Result<Stmt, ParseError> {
        self.advance();
        let vals = match self.peek() {
            Token::End | Token::Else | Token::ElseIf | Token::Until | Token::Eof | Token::Semicolon => Vec::new(),
            _ => self.parse_expr_list()?,
        };
        self.consume_if(&Token::Semicolon);
        Ok(Stmt::Return(vals))
    }

    fn parse_match(&mut self) -> Result<Stmt, ParseError> {
        self.advance();
        let expr = self.parse_expr(0)?;
        self.expect(&Token::LBrace)?;
        let mut arms = Vec::new();
        while !self.check(&Token::RBrace) && !self.check(&Token::Eof) {
            self.consume_if(&Token::Case);
            let pattern = self.parse_match_pattern()?;
            self.expect(&Token::Arrow)?;
            let body = if self.check(&Token::LBrace) {
                self.advance();
                let b = self.parse_block()?;
                self.expect(&Token::RBrace)?;
                b
            } else {
                vec![self.parse_stmt()?]
            };
            arms.push(MatchArm { pattern, body });
            self.consume_if(&Token::Comma);
        }
        self.expect(&Token::RBrace)?;
        Ok(Stmt::Match { expr, arms })
    }

    fn parse_match_pattern(&mut self) -> Result<MatchPattern, ParseError> {
        match self.peek().clone() {
            Token::Ident(s) if s == "_" => { self.advance(); Ok(MatchPattern::Wildcard) }
            Token::Default => { self.advance(); Ok(MatchPattern::Wildcard) }
            _ => Ok(MatchPattern::Literal(self.parse_expr(0)?)),
        }
    }

    fn parse_import(&mut self) -> Result<Stmt, ParseError> {
        self.advance();
        let path = match self.advance().clone() {
            Token::Str(s) => s,
            Token::Ident(s) => s,
            t => return Err(ParseError { message: format!("expected module path, got {:?}", t), line: self.peek_span_line() }),
        };
        let alias = if self.consume_if(&Token::Ident("as".to_string())) {
            Some(self.expect_ident()?)
        } else { None };
        Ok(Stmt::Import { path, alias })
    }

    fn parse_expr_stmt(&mut self) -> Result<Stmt, ParseError> {
        let expr = self.parse_expr(0)?;

        // Check for assignment
        if self.check(&Token::Eq) || self.check(&Token::Comma) {
            let mut targets = vec![expr];
            while self.consume_if(&Token::Comma) {
                targets.push(self.parse_expr(0)?);
            }
            self.expect(&Token::Eq)?;
            let values = self.parse_expr_list()?;
            return Ok(Stmt::Assign { target: targets, value: values });
        }

        // Compound assignments
        let op = match self.peek() {
            Token::PlusEq  => Some(BinOp::Add),
            Token::MinusEq => Some(BinOp::Sub),
            Token::StarEq  => Some(BinOp::Mul),
            Token::SlashEq => Some(BinOp::Div),
            _ => None,
        };
        if let Some(op) = op {
            self.advance();
            let value = self.parse_expr(0)?;
            return Ok(Stmt::CompoundAssign { target: expr, op, value });
        }

        Ok(Stmt::Call(expr))
    }

    fn parse_expr_list(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut exprs = vec![self.parse_expr(0)?];
        while self.consume_if(&Token::Comma) {
            exprs.push(self.parse_expr(0)?);
        }
        Ok(exprs)
    }

    // ── Expression parsing (Pratt) ────────────────────────────────────────

    fn parse_expr(&mut self, min_prec: u8) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_unary()?;

        loop {
            let op = match self.peek() {
                Token::Plus     => Some(BinOp::Add),
                Token::Minus    => Some(BinOp::Sub),
                Token::Star     => Some(BinOp::Mul),
                Token::Slash    => Some(BinOp::Div),
                Token::SlashSlash => Some(BinOp::IDiv),
                Token::Percent  => Some(BinOp::Mod),
                Token::Caret    => Some(BinOp::Pow),
                Token::DotDot   => Some(BinOp::Concat),
                Token::EqEq     => Some(BinOp::Eq),
                Token::NotEq    => Some(BinOp::NotEq),
                Token::Lt       => Some(BinOp::Lt),
                Token::LtEq     => Some(BinOp::LtEq),
                Token::Gt       => Some(BinOp::Gt),
                Token::GtEq     => Some(BinOp::GtEq),
                Token::And      => Some(BinOp::And),
                Token::Or       => Some(BinOp::Or),
                Token::Amp      => Some(BinOp::BitAnd),
                Token::Pipe     => Some(BinOp::BitOr),
                Token::Tilde    => Some(BinOp::BitXor),
                Token::ShiftLeft  => Some(BinOp::Shl),
                Token::ShiftRight => Some(BinOp::Shr),
                _ => None,
            };
            if let Some(op) = op {
                let prec = op.precedence();
                if prec <= min_prec { break; }
                self.advance();
                let next_min = if op.is_right_assoc() { prec - 1 } else { prec };
                let rhs = self.parse_expr(next_min)?;
                lhs = Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) };
            } else { break; }
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        match self.peek().clone() {
            Token::Minus => { self.advance(); let e = self.parse_unary()?; Ok(Expr::Unary { op: UnOp::Neg, expr: Box::new(e) }) }
            Token::Not | Token::Bang => { self.advance(); let e = self.parse_unary()?; Ok(Expr::Unary { op: UnOp::Not, expr: Box::new(e) }) }
            Token::Hash  => { self.advance(); let e = self.parse_unary()?; Ok(Expr::Unary { op: UnOp::Len, expr: Box::new(e) }) }
            Token::Tilde => { self.advance(); let e = self.parse_unary()?; Ok(Expr::Unary { op: UnOp::BitNot, expr: Box::new(e) }) }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.peek().clone() {
                Token::Dot => {
                    self.advance();
                    let name = self.expect_ident()?;
                    // Check if method call
                    if self.check(&Token::LParen) {
                        let args = self.parse_args()?;
                        expr = Expr::MethodCall { obj: Box::new(expr), method: name, args };
                    } else {
                        expr = Expr::Field { table: Box::new(expr), name };
                    }
                }
                Token::Colon => {
                    self.advance();
                    let method = self.expect_ident()?;
                    let args = self.parse_args()?;
                    expr = Expr::MethodCall { obj: Box::new(expr), method, args };
                }
                Token::LBracket => {
                    self.advance();
                    let key = self.parse_expr(0)?;
                    self.expect(&Token::RBracket)?;
                    expr = Expr::Index { table: Box::new(expr), key: Box::new(key) };
                }
                Token::LParen | Token::LBrace | Token::Str(_) => {
                    let args = self.parse_args()?;
                    expr = Expr::Call { callee: Box::new(expr), args };
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_args(&mut self) -> Result<Vec<Expr>, ParseError> {
        match self.peek().clone() {
            Token::LParen => {
                self.advance();
                if self.check(&Token::RParen) { self.advance(); return Ok(Vec::new()); }
                let args = self.parse_expr_list()?;
                self.expect(&Token::RParen)?;
                Ok(args)
            }
            Token::LBrace => {
                let t = self.parse_table_ctor()?;
                Ok(vec![t])
            }
            Token::Str(s) => { self.advance(); Ok(vec![Expr::Str(s)]) }
            t => Err(ParseError { message: format!("expected args, got {:?}", t), line: self.peek_span_line() })
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        match self.peek().clone() {
            Token::Nil       => { self.advance(); Ok(Expr::Nil) }
            Token::True      => { self.advance(); Ok(Expr::Bool(true)) }
            Token::False     => { self.advance(); Ok(Expr::Bool(false)) }
            Token::Int(v)    => { self.advance(); Ok(Expr::Int(v)) }
            Token::Float(v)  => { self.advance(); Ok(Expr::Float(v)) }
            Token::Str(s)    => { self.advance(); Ok(Expr::Str(s)) }
            Token::DotDotDot => { self.advance(); Ok(Expr::Vararg) }
            Token::Ident(s)  => { self.advance(); Ok(Expr::Ident(s)) }
            Token::LParen    => {
                self.advance();
                let e = self.parse_expr(0)?;
                self.expect(&Token::RParen)?;
                Ok(e)
            }
            Token::LBrace    => self.parse_table_ctor(),
            Token::Function  => {
                self.advance();
                let (params, vararg, body) = self.parse_func_body()?;
                Ok(Expr::FuncExpr { params, vararg, body })
            }
            t => Err(ParseError { message: format!("unexpected token: {:?}", t), line: self.peek_span_line() })
        }
    }

    fn parse_table_ctor(&mut self) -> Result<Expr, ParseError> {
        self.expect(&Token::LBrace)?;
        let mut fields = Vec::new();
        while !self.check(&Token::RBrace) && !self.check(&Token::Eof) {
            let field = if self.check(&Token::LBracket) {
                self.advance();
                let key = self.parse_expr(0)?;
                self.expect(&Token::RBracket)?;
                self.expect(&Token::Eq)?;
                let val = self.parse_expr(0)?;
                TableField::ExprKey(key, val)
            } else if let Token::Ident(name) = self.peek().clone() {
                if self.tokens.get(self.pos + 1).map(|t| &t.token) == Some(&Token::Eq) {
                    self.advance(); self.advance();
                    let val = self.parse_expr(0)?;
                    TableField::NameKey(name, val)
                } else {
                    TableField::Value(self.parse_expr(0)?)
                }
            } else {
                TableField::Value(self.parse_expr(0)?)
            };
            fields.push(field);
            if !self.consume_if(&Token::Comma) && !self.consume_if(&Token::Semicolon) { break; }
        }
        self.expect(&Token::RBrace)?;
        Ok(Expr::TableCtor(fields))
    }
}

/// Parse a script from source text (name, source order).
pub fn parse_named(name: &str, source: &str) -> Result<Script, ParseError> {
    parse(source, name)
}

/// Parse a script from source text.
pub fn parse(source: &str, name: &str) -> Result<Script, ParseError> {
    let mut lexer = super::lexer::Lexer::new(source);
    let tokens = lexer.tokenize();
    let mut parser = Parser::new(tokens);
    parser.parse_script(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(src: &str) -> Script {
        parse(src, "test").unwrap_or_else(|e| panic!("Parse failed: {}", e))
    }

    #[test]
    fn test_parse_local_assign() {
        let s = parse_ok("local x = 42");
        assert_eq!(s.stmts.len(), 1);
        assert!(matches!(&s.stmts[0], Stmt::LocalDecl { name, init: Some(Expr::Int(42)) } if name == "x"));
    }

    #[test]
    fn test_parse_function() {
        let s = parse_ok("function greet(name) return name end");
        assert!(matches!(&s.stmts[0], Stmt::FuncDecl { .. }));
    }

    #[test]
    fn test_parse_if_else() {
        let s = parse_ok("if x > 0 then return 1 else return -1 end");
        assert!(matches!(&s.stmts[0], Stmt::If { .. }));
    }

    #[test]
    fn test_parse_while() {
        let s = parse_ok("while i < 10 do i = i + 1 end");
        assert!(matches!(&s.stmts[0], Stmt::While { .. }));
    }

    #[test]
    fn test_parse_table_ctor() {
        let s = parse_ok("local t = {x = 1, y = 2, 3}");
        assert!(matches!(&s.stmts[0], Stmt::LocalDecl { init: Some(Expr::TableCtor(_)), .. }));
    }

    #[test]
    fn test_parse_binary_expr() {
        let s = parse_ok("local z = a + b * c");
        if let Stmt::LocalDecl { init: Some(expr), .. } = &s.stmts[0] {
            assert!(matches!(expr, Expr::Binary { op: BinOp::Add, .. }));
        }
    }

    #[test]
    fn test_parse_for_numeric() {
        let s = parse_ok("for i = 1, 10, 2 do end");
        assert!(matches!(&s.stmts[0], Stmt::NumericFor { var, .. } if var == "i"));
    }

    #[test]
    fn test_parse_error() {
        let result = parse("local = 123", "test");
        assert!(result.is_err());
    }
}
