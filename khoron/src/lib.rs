use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Nil,
    Number(f64),
    String(String),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Nil => write!(f, "nil"),
            Value::Number(value) => {
                if value.fract() == 0.0 {
                    write!(f, "{value:.0}")
                } else {
                    write!(f, "{value}")
                }
            }
            Value::String(value) => write!(f, "{value}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Ident(String),
    Number(f64),
    String(String),
    Equal,
    Plus,
    Minus,
    Star,
    Slash,
    LeftParen,
    RightParen,
    Comma,
    Newline,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
enum Expr {
    Literal(Value),
    Variable(String),
    Call {
        name: String,
        args: Vec<Expr>,
    },
    Binary {
        left: Box<Expr>,
        op: Token,
        right: Box<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq)]
enum Stmt {
    Assign { name: String, expr: Expr },
    Expr(Expr),
}

#[derive(Debug, Default)]
pub struct Runtime {
    globals: HashMap<String, Value>,
    output: Vec<String>,
}

impl Runtime {
    pub fn run(&mut self, source: &str) -> Result<&[String], String> {
        let tokens = lex(source)?;
        let mut parser = Parser::new(tokens);
        let program = parser.parse_program()?;
        for stmt in program {
            self.eval_stmt(&stmt)?;
        }
        Ok(&self.output)
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.globals.get(name)
    }

    fn eval_stmt(&mut self, stmt: &Stmt) -> Result<Value, String> {
        match stmt {
            Stmt::Assign { name, expr } => {
                let value = self.eval(expr)?;
                self.globals.insert(name.clone(), value.clone());
                Ok(value)
            }
            Stmt::Expr(expr) => self.eval(expr),
        }
    }

    fn eval(&mut self, expr: &Expr) -> Result<Value, String> {
        match expr {
            Expr::Literal(value) => Ok(value.clone()),
            Expr::Variable(name) => Ok(self.globals.get(name).cloned().unwrap_or(Value::Nil)),
            Expr::Call { name, args } => {
                let values = args
                    .iter()
                    .map(|arg| self.eval(arg))
                    .collect::<Result<Vec<_>, _>>()?;
                match name.as_str() {
                    "print" => {
                        let line = values
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(" ");
                        self.output.push(line);
                        Ok(Value::Nil)
                    }
                    _ => Err(format!("unknown function `{name}`")),
                }
            }
            Expr::Binary { left, op, right } => {
                let left = self.eval(left)?;
                let right = self.eval(right)?;
                eval_binary(left, op, right)
            }
        }
    }
}

fn eval_binary(left: Value, op: &Token, right: Value) -> Result<Value, String> {
    match op {
        Token::Plus => match (left, right) {
            (Value::Number(left), Value::Number(right)) => Ok(Value::Number(left + right)),
            (left, right) => Ok(Value::String(format!("{left}{right}"))),
        },
        Token::Minus => number_pair(left, right).map(|(left, right)| Value::Number(left - right)),
        Token::Star => number_pair(left, right).map(|(left, right)| Value::Number(left * right)),
        Token::Slash => number_pair(left, right).map(|(left, right)| Value::Number(left / right)),
        _ => Err("unsupported operator".to_string()),
    }
}

fn number_pair(left: Value, right: Value) -> Result<(f64, f64), String> {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => Ok((left, right)),
        _ => Err("operator expects numbers".to_string()),
    }
}

fn lex(source: &str) -> Result<Vec<Token>, String> {
    let mut chars = source.chars().peekable();
    let mut tokens = Vec::new();
    while let Some(ch) = chars.next() {
        match ch {
            ' ' | '\r' | '\t' => {}
            '\n' | ';' => tokens.push(Token::Newline),
            '=' => tokens.push(Token::Equal),
            '+' => tokens.push(Token::Plus),
            '-' => {
                if chars.peek() == Some(&'-') {
                    for ch in chars.by_ref() {
                        if ch == '\n' {
                            tokens.push(Token::Newline);
                            break;
                        }
                    }
                } else {
                    tokens.push(Token::Minus);
                }
            }
            '*' => tokens.push(Token::Star),
            '/' => tokens.push(Token::Slash),
            '(' => tokens.push(Token::LeftParen),
            ')' => tokens.push(Token::RightParen),
            ',' => tokens.push(Token::Comma),
            '"' | '\'' => tokens.push(Token::String(read_string(&mut chars, ch)?)),
            ch if ch.is_ascii_digit() => tokens.push(read_number(ch, &mut chars)?),
            ch if is_ident_start(ch) => tokens.push(read_ident(ch, &mut chars)),
            _ => return Err(format!("unexpected character `{ch}`")),
        }
    }
    tokens.push(Token::Eof);
    Ok(tokens)
}

fn read_string(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    quote: char,
) -> Result<String, String> {
    let mut out = String::new();
    while let Some(ch) = chars.next() {
        match ch {
            ch if ch == quote => return Ok(out),
            '\\' => match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(ch) => out.push(ch),
                None => return Err("unterminated escape".to_string()),
            },
            ch => out.push(ch),
        }
    }
    Err("unterminated string".to_string())
}

fn read_number(
    first: char,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Result<Token, String> {
    let mut text = String::from(first);
    while let Some(ch) = chars.peek() {
        if ch.is_ascii_digit() || *ch == '.' {
            text.push(*ch);
            chars.next();
        } else {
            break;
        }
    }
    text.parse::<f64>()
        .map(Token::Number)
        .map_err(|_| format!("invalid number `{text}`"))
}

fn read_ident(first: char, chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Token {
    let mut text = String::from(first);
    while let Some(ch) = chars.peek() {
        if ch.is_ascii_alphanumeric() || *ch == '_' {
            text.push(*ch);
            chars.next();
        } else {
            break;
        }
    }
    Token::Ident(text)
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn parse_program(&mut self) -> Result<Vec<Stmt>, String> {
        let mut stmts = Vec::new();
        while !self.at(&Token::Eof) {
            self.skip_newlines();
            if self.at(&Token::Eof) {
                break;
            }
            stmts.push(self.statement()?);
            self.skip_newlines();
        }
        Ok(stmts)
    }

    fn statement(&mut self) -> Result<Stmt, String> {
        if let Token::Ident(name) = self.peek().clone()
            && self.peek_next() == &Token::Equal
        {
            self.advance();
            self.advance();
            return Ok(Stmt::Assign {
                name,
                expr: self.expression()?,
            });
        }
        Ok(Stmt::Expr(self.expression()?))
    }

    fn expression(&mut self) -> Result<Expr, String> {
        self.term()
    }

    fn term(&mut self) -> Result<Expr, String> {
        let mut expr = self.factor()?;
        while self.at(&Token::Plus) || self.at(&Token::Minus) {
            let op = self.advance().clone();
            let right = self.factor()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn factor(&mut self) -> Result<Expr, String> {
        let mut expr = self.primary()?;
        while self.at(&Token::Star) || self.at(&Token::Slash) {
            let op = self.advance().clone();
            let right = self.primary()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn primary(&mut self) -> Result<Expr, String> {
        match self.advance().clone() {
            Token::Number(value) => Ok(Expr::Literal(Value::Number(value))),
            Token::String(value) => Ok(Expr::Literal(Value::String(value))),
            Token::Ident(name) => {
                if self.at(&Token::LeftParen) {
                    self.advance();
                    let mut args = Vec::new();
                    if !self.at(&Token::RightParen) {
                        loop {
                            args.push(self.expression()?);
                            if !self.at(&Token::Comma) {
                                break;
                            }
                            self.advance();
                        }
                    }
                    self.expect(Token::RightParen)?;
                    Ok(Expr::Call { name, args })
                } else {
                    Ok(Expr::Variable(name))
                }
            }
            Token::LeftParen => {
                let expr = self.expression()?;
                self.expect(Token::RightParen)?;
                Ok(expr)
            }
            token => Err(format!("expected expression, found {token:?}")),
        }
    }

    fn skip_newlines(&mut self) {
        while self.at(&Token::Newline) {
            self.advance();
        }
    }

    fn expect(&mut self, token: Token) -> Result<(), String> {
        if self.at(&token) {
            self.advance();
            Ok(())
        } else {
            Err(format!("expected {token:?}, found {:?}", self.peek()))
        }
    }

    fn at(&self, token: &Token) -> bool {
        self.peek() == token
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn peek_next(&self) -> &Token {
        self.tokens.get(self.pos + 1).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) -> &Token {
        let pos = self.pos;
        self.pos += 1;
        self.tokens.get(pos).unwrap_or(&Token::Eof)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globals_are_shared_between_assignments() {
        let mut runtime = Runtime::default();
        runtime.run("x = 4\ny = x + 6").unwrap();
        assert_eq!(runtime.get("y"), Some(&Value::Number(10.0)));
    }

    #[test]
    fn plus_concatenates_strings() {
        let mut runtime = Runtime::default();
        let output = runtime.run("name = 'bot'\nprint('crop' + name)").unwrap();
        assert_eq!(output, &["cropbot".to_string()]);
    }

    #[test]
    fn arithmetic_still_uses_plus_for_numbers() {
        let mut runtime = Runtime::default();
        let output = runtime.run("print(2 + 3 * 4)").unwrap();
        assert_eq!(output, &["14".to_string()]);
    }
}
