//! Lexer — tokenizes script source into a token stream.

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Literals
    Nil,
    True,
    False,
    Int(i64),
    Float(f64),
    Str(String),
    Ident(String),

    // Keywords
    And, Or, Not,
    If, Then, Else, ElseIf, End,
    While, Do, For, In, Repeat, Until,
    Function, Return, Local, Break, Continue,
    Class, Self_, New, Import, Export,
    Match, Case, Default,

    // Operators
    Plus, Minus, Star, Slash, Percent, Caret, Hash,
    Amp, Pipe, Tilde, ShiftLeft, ShiftRight, SlashSlash,
    EqEq, NotEq, Lt, LtEq, Gt, GtEq,
    Eq, PlusEq, MinusEq, StarEq, SlashEq,
    DotDot, DotDotDot, Arrow,
    Bang,

    // Delimiters
    LParen, RParen, LBrace, RBrace, LBracket, RBracket,
    Comma, Semicolon, Colon, ColonColon, Dot,

    // Meta
    Eof,
}

#[derive(Debug, Clone)]
pub struct Span {
    pub line:   u32,
    pub column: u32,
}

#[derive(Debug, Clone)]
pub struct TokenWithSpan {
    pub token: Token,
    pub span:  Span,
}

/// Tokenizes source code into a flat Vec of tokens.
pub struct Lexer {
    source:  Vec<char>,
    pos:     usize,
    line:    u32,
    column:  u32,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Self { source: source.chars().collect(), pos: 0, line: 1, column: 1 }
    }

    fn peek(&self) -> Option<char> { self.source.get(self.pos).copied() }
    fn peek2(&self) -> Option<char> { self.source.get(self.pos + 1).copied() }

    fn advance(&mut self) -> Option<char> {
        let c = self.source.get(self.pos).copied();
        if let Some(ch) = c {
            self.pos += 1;
            if ch == '\n' { self.line += 1; self.column = 1; }
            else { self.column += 1; }
        }
        c
    }

    fn span(&self) -> Span { Span { line: self.line, column: self.column } }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            // Skip whitespace
            while self.peek().map(|c| c.is_whitespace()).unwrap_or(false) {
                self.advance();
            }
            // Skip line comments  (-- or //)
            if self.peek() == Some('-') && self.peek2() == Some('-') {
                self.advance(); self.advance();
                while self.peek().map(|c| c != '\n').unwrap_or(false) { self.advance(); }
                continue;
            }
            if self.peek() == Some('/') && self.peek2() == Some('/') {
                self.advance(); self.advance();
                while self.peek().map(|c| c != '\n').unwrap_or(false) { self.advance(); }
                continue;
            }
            // Skip block comments (/* */ or --[[ ]])
            if self.peek() == Some('/') && self.peek2() == Some('*') {
                self.advance(); self.advance();
                while self.pos + 1 < self.source.len() {
                    if self.peek() == Some('*') && self.peek2() == Some('/') {
                        self.advance(); self.advance(); break;
                    }
                    self.advance();
                }
                continue;
            }
            break;
        }
    }

    fn read_string(&mut self, delim: char) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c == delim { self.advance(); break; }
            if c == '\\' {
                self.advance();
                match self.advance() {
                    Some('n')  => s.push('\n'),
                    Some('t')  => s.push('\t'),
                    Some('r')  => s.push('\r'),
                    Some('\\') => s.push('\\'),
                    Some('\'') => s.push('\''),
                    Some('"')  => s.push('"'),
                    Some('0')  => s.push('\0'),
                    Some(x)    => { s.push('\\'); s.push(x); }
                    None       => break,
                }
            } else {
                s.push(c);
                self.advance();
            }
        }
        s
    }

    fn read_number(&mut self, first: char) -> Token {
        let mut num = first.to_string();
        let mut is_float = false;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() { num.push(c); self.advance(); }
            else if c == '.' && !is_float && self.peek2().map(|n| n.is_ascii_digit()).unwrap_or(false) {
                is_float = true; num.push(c); self.advance();
            }
            else if (c == 'e' || c == 'E') && !num.contains('e') && !num.contains('E') {
                is_float = true; num.push(c); self.advance();
                if self.peek() == Some('+') || self.peek() == Some('-') {
                    if let Some(sign) = self.advance() { num.push(sign); }
                }
            }
            else { break; }
        }
        if is_float {
            Token::Float(num.parse().unwrap_or(0.0))
        } else {
            Token::Int(num.parse().unwrap_or(0))
        }
    }

    fn read_ident(&mut self, first: char) -> Token {
        let mut ident = first.to_string();
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' { ident.push(c); self.advance(); }
            else { break; }
        }
        match ident.as_str() {
            "nil"      => Token::Nil,
            "true"     => Token::True,
            "false"    => Token::False,
            "and"      => Token::And,
            "or"       => Token::Or,
            "not"      => Token::Not,
            "if"       => Token::If,
            "then"     => Token::Then,
            "else"     => Token::Else,
            "elseif"   => Token::ElseIf,
            "end"      => Token::End,
            "while"    => Token::While,
            "do"       => Token::Do,
            "for"      => Token::For,
            "in"       => Token::In,
            "repeat"   => Token::Repeat,
            "until"    => Token::Until,
            "function" => Token::Function,
            "return"   => Token::Return,
            "local"    => Token::Local,
            "break"    => Token::Break,
            "continue" => Token::Continue,
            "class"    => Token::Class,
            "self"     => Token::Self_,
            "new"      => Token::New,
            "import"   => Token::Import,
            "export"   => Token::Export,
            "match"    => Token::Match,
            "case"     => Token::Case,
            "default"  => Token::Default,
            _          => Token::Ident(ident),
        }
    }

    pub fn tokenize(&mut self) -> Vec<TokenWithSpan> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            let span = self.span();
            let ch = match self.advance() {
                Some(c) => c,
                None    => { tokens.push(TokenWithSpan { token: Token::Eof, span }); break; }
            };

            let token = match ch {
                '+' => { if self.peek() == Some('=') { self.advance(); Token::PlusEq  } else { Token::Plus  } }
                '-' => { if self.peek() == Some('=') { self.advance(); Token::MinusEq } else if self.peek() == Some('>') { self.advance(); Token::Arrow } else { Token::Minus } }
                '*' => { if self.peek() == Some('=') { self.advance(); Token::StarEq  } else { Token::Star  } }
                '/' => { if self.peek() == Some('=') { self.advance(); Token::SlashEq } else if self.peek() == Some('/') { self.advance(); Token::SlashSlash } else { Token::Slash } }
                '%' => Token::Percent,
                '^' => Token::Caret,
                '#' => Token::Hash,
                '&' => Token::Amp,
                '|' => Token::Pipe,
                '~' => { if self.peek() == Some('=') { self.advance(); Token::NotEq } else { Token::Tilde } }
                '<' => { if self.peek() == Some('=') { self.advance(); Token::LtEq } else if self.peek() == Some('<') { self.advance(); Token::ShiftLeft } else { Token::Lt } }
                '>' => { if self.peek() == Some('=') { self.advance(); Token::GtEq } else if self.peek() == Some('>') { self.advance(); Token::ShiftRight } else { Token::Gt } }
                '=' => { if self.peek() == Some('=') { self.advance(); Token::EqEq } else { Token::Eq } }
                '!' => { if self.peek() == Some('=') { self.advance(); Token::NotEq } else { Token::Bang } }
                '.' => {
                    if self.peek() == Some('.') {
                        self.advance();
                        if self.peek() == Some('.') { self.advance(); Token::DotDotDot }
                        else { Token::DotDot }
                    } else { Token::Dot }
                }
                ':' => { if self.peek() == Some(':') { self.advance(); Token::ColonColon } else { Token::Colon } }
                '(' => Token::LParen,
                ')' => Token::RParen,
                '{' => Token::LBrace,
                '}' => Token::RBrace,
                '[' => Token::LBracket,
                ']' => Token::RBracket,
                ',' => Token::Comma,
                ';' => Token::Semicolon,
                '\'' | '"' => Token::Str(self.read_string(ch)),
                '`' => Token::Str(self.read_string('`')),
                c if c.is_ascii_digit() => self.read_number(c),
                c if c.is_alphabetic() || c == '_' => self.read_ident(c),
                _ => continue,
            };
            tokens.push(TokenWithSpan { token, span });
        }
        tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(src: &str) -> Vec<Token> {
        let mut l = Lexer::new(src);
        l.tokenize().into_iter().map(|t| t.token).collect()
    }

    #[test]
    fn test_lex_simple_assign() {
        let toks = lex("local x = 42");
        assert!(toks.contains(&Token::Local));
        assert!(toks.contains(&Token::Ident("x".to_string())));
        assert!(toks.contains(&Token::Eq));
        assert!(toks.contains(&Token::Int(42)));
    }

    #[test]
    fn test_lex_string() {
        let toks = lex(r#"local s = "hello world""#);
        assert!(toks.contains(&Token::Str("hello world".to_string())));
    }

    #[test]
    fn test_lex_float() {
        let toks = lex("3.14");
        assert!(toks.iter().any(|t| matches!(t, Token::Float(v) if (*v - 3.14).abs() < 1e-6)));
    }

    #[test]
    fn test_lex_operators() {
        let toks = lex("a == b ~= c <= d >= e");
        assert!(toks.contains(&Token::EqEq));
        assert!(toks.contains(&Token::NotEq));
        assert!(toks.contains(&Token::LtEq));
        assert!(toks.contains(&Token::GtEq));
    }

    #[test]
    fn test_lex_keywords() {
        let toks = lex("if x then return end");
        assert!(toks.contains(&Token::If));
        assert!(toks.contains(&Token::Then));
        assert!(toks.contains(&Token::Return));
        assert!(toks.contains(&Token::End));
    }

    #[test]
    fn test_lex_comment_skip() {
        let toks = lex("local x = 1 -- this is a comment\nlocal y = 2");
        assert!(!toks.iter().any(|t| matches!(t, Token::Ident(s) if s == "this")));
    }
}
