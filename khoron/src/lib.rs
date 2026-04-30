use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub enum EvalError {
    Return(Value),
    Error(String),
}

impl From<String> for EvalError {
    fn from(err: String) -> Self {
        EvalError::Error(err)
    }
}

impl From<&str> for EvalError {
    fn from(err: &str) -> Self {
        EvalError::Error(err.to_string())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Nil,
    Bool(bool),
    Number(f64),
    String(String),
    List(Rc<RefCell<Vec<Value>>>),
    Dict(Rc<RefCell<HashMap<String, Value>>>),
    Function(Rc<UserFunction>),
    NativeFunction(fn(Vec<Value>) -> Result<Value, String>),
    Class(Rc<Class>),
    Instance(Rc<RefCell<Instance>>),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Nil => write!(f, "nil"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Number(n) => {
                if n.fract() == 0.0 {
                    write!(f, "{n:.0}")
                } else {
                    write!(f, "{n}")
                }
            }
            Value::String(s) => write!(f, "{s}"),
            Value::List(l) => {
                let l = l.borrow();
                write!(f, "[")?;
                for (i, v) in l.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, "]")
            }
            Value::Dict(d) => {
                let d = d.borrow();
                write!(f, "{{")?;
                for (i, (k, v)) in d.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "\"{k}\": {v}")?;
                }
                write!(f, "}}")
            }
            Value::Function(func) => write!(f, "<function {}>", func.name),
            Value::NativeFunction(_) => write!(f, "<native fn>"),
            Value::Class(class) => write!(f, "<class {}>", class.name),
            Value::Instance(inst) => write!(f, "<instance of {}>", inst.borrow().class.name),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UserFunction {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
    pub closure: Option<Rc<RefCell<Environment>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Class {
    pub name: String,
    pub methods: HashMap<String, Rc<UserFunction>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    pub class: Rc<Class>,
    pub fields: HashMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Ident(String),
    Number(f64),
    String(String),
    Equal,
    EqualEqual,
    BangEqual,
    Plus,
    PlusPlus,
    PlusEqual,
    Minus,
    MinusMinus,
    MinusEqual,
    Star,
    StarEqual,
    Slash,
    SlashEqual,
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    LeftBrace,
    RightBrace,
    Comma,
    Dot,
    Colon,
    Newline,
    Function,
    End,
    Then,
    Do,
    Class,
    If,
    Else,
    While,
    Return,
    True,
    False,
    SelfKw,
    Eof,
}

fn format_token(kind: &TokenKind) -> String {
    match kind {
        TokenKind::Ident(s) => format!("'{s}'"),
        TokenKind::Number(n) => format!("'{n}'"),
        TokenKind::String(s) => format!("\"{s}\""),
        TokenKind::Equal => "'='".to_string(),
        TokenKind::EqualEqual => "'=='".to_string(),
        TokenKind::BangEqual => "'!='".to_string(),
        TokenKind::Plus => "'+'".to_string(),
        TokenKind::PlusPlus => "'++'".to_string(),
        TokenKind::PlusEqual => "'+='".to_string(),
        TokenKind::Minus => "'-'".to_string(),
        TokenKind::MinusMinus => "'--'".to_string(),
        TokenKind::MinusEqual => "'-='".to_string(),
        TokenKind::Star => "'*'".to_string(),
        TokenKind::StarEqual => "'*='".to_string(),
        TokenKind::Slash => "'/'".to_string(),
        TokenKind::SlashEqual => "'/='".to_string(),
        TokenKind::LeftParen => "'('".to_string(),
        TokenKind::RightParen => "')'".to_string(),
        TokenKind::LeftBracket => "'['".to_string(),
        TokenKind::RightBracket => "']'".to_string(),
        TokenKind::LeftBrace => "'{{'".to_string(),
        TokenKind::RightBrace => "'}}'".to_string(),
        TokenKind::Comma => "','".to_string(),
        TokenKind::Dot => "'.'".to_string(),
        TokenKind::Colon => "':'".to_string(),
        TokenKind::Newline => "newline".to_string(),
        TokenKind::Function => "'function'".to_string(),
        TokenKind::End => "'end'".to_string(),
        TokenKind::Then => "'then'".to_string(),
        TokenKind::Do => "'do'".to_string(),
        TokenKind::Class => "'class'".to_string(),
        TokenKind::If => "'if'".to_string(),
        TokenKind::Else => "'else'".to_string(),
        TokenKind::While => "'while'".to_string(),
        TokenKind::Return => "'return'".to_string(),
        TokenKind::True => "'true'".to_string(),
        TokenKind::False => "'false'".to_string(),
        TokenKind::SelfKw => "'self'".to_string(),
        TokenKind::Eof => "end of file".to_string(),
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Token {
    kind: TokenKind,
    line: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Value),
    Variable(String),
    List(Vec<Expr>),
    Dict(Vec<(Expr, Expr)>),
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    Get {
        object: Box<Expr>,
        name: String,
    },
    Set {
        object: Box<Expr>,
        name: String,
        value: Box<Expr>,
    },
    Index {
        target: Box<Expr>,
        index: Box<Expr>,
    },
    SetIndex {
        target: Box<Expr>,
        index: Box<Expr>,
        value: Box<Expr>,
    },
    Binary {
        left: Box<Expr>,
        op: TokenKind,
        right: Box<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Assign { name: String, expr: Expr },
    Expr(Expr),
    FnDef(Rc<UserFunction>),
    ClassDef(Rc<Class>),
    If { condition: Expr, then_branch: Vec<Stmt>, else_branch: Option<Vec<Stmt>> },
    While { condition: Expr, body: Vec<Stmt> },
    Return(Option<Expr>),
}

#[derive(Debug, Default)]
pub struct Environment {
    parent: Option<Rc<RefCell<Environment>>>,
    values: HashMap<String, Value>,
}

impl PartialEq for Environment {
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}

impl Environment {
    fn new(parent: Option<Rc<RefCell<Environment>>>) -> Self {
        Self { parent, values: HashMap::new() }
    }

    fn define(&mut self, name: String, value: Value) {
        self.values.insert(name, value);
    }

    fn get(&self, name: &str) -> Option<Value> {
        if let Some(val) = self.values.get(name) {
            Some(val.clone())
        } else if let Some(parent) = &self.parent {
            parent.borrow().get(name)
        } else {
            None
        }
    }

    fn assign(&mut self, name: &str, value: Value) -> bool {
        if self.values.contains_key(name) {
            self.values.insert(name.to_string(), value);
            true
        } else if let Some(parent) = &self.parent {
            parent.borrow_mut().assign(name, value)
        } else {
            false
        }
    }
}

pub struct Runtime {
    env: Rc<RefCell<Environment>>,
    output: Vec<String>,
}

impl Default for Runtime {
    fn default() -> Self {
        let env = Rc::new(RefCell::new(Environment::default()));
        env.borrow_mut().define("print".to_string(), Value::NativeFunction(|_args| {
            Ok(Value::Nil)
        }));
        Self { env, output: Vec::new() }
    }
}

impl Runtime {
    pub fn run(&mut self, source: &str) -> Result<&[String], String> {
        let tokens = lex(source)?;
        let mut parser = Parser::new(tokens);
        let program = parser.parse_program()?;
        for stmt in program {
            match self.eval_stmt(&stmt, self.env.clone()) {
                Ok(_) => {}
                Err(EvalError::Return(_)) => break,
                Err(EvalError::Error(err)) => return Err(err),
            }
        }
        Ok(&self.output)
    }

    fn eval_stmt(&mut self, stmt: &Stmt, env: Rc<RefCell<Environment>>) -> Result<Value, EvalError> {
        match stmt {
            Stmt::Assign { name, expr } => {
                let value = self.eval_expr(expr, env.clone())?;
                if !env.borrow_mut().assign(name, value.clone()) {
                    env.borrow_mut().define(name.clone(), value.clone());
                }
                Ok(value)
            }
            Stmt::Expr(expr) => self.eval_expr(expr, env),
            Stmt::FnDef(func) => {
                let mut f = (**func).clone();
                f.closure = Some(env.clone());
                let name = f.name.clone();
                let value = Value::Function(Rc::new(f));
                env.borrow_mut().define(name, value.clone());
                Ok(value)
            }
            Stmt::ClassDef(class) => {
                let value = Value::Class(class.clone());
                env.borrow_mut().define(class.name.clone(), value.clone());
                Ok(value)
            }
            Stmt::If { condition, then_branch, else_branch } => {
                let cond_val = self.eval_expr(condition, env.clone())?;
                if Runtime::is_truthy(&cond_val) {
                    for stmt in then_branch {
                        self.eval_stmt(stmt, env.clone())?;
                    }
                } else if let Some(else_branch) = else_branch {
                    for stmt in else_branch {
                        self.eval_stmt(stmt, env.clone())?;
                    }
                }
                Ok(Value::Nil)
            }
            Stmt::While { condition, body } => {
                while {
                    let cond_val = self.eval_expr(condition, env.clone())?;
                    Runtime::is_truthy(&cond_val)
                } {
                    for stmt in body {
                        self.eval_stmt(stmt, env.clone())?;
                    }
                }
                Ok(Value::Nil)
            }
            Stmt::Return(expr) => {
                let val = if let Some(e) = expr {
                    self.eval_expr(e, env)?
                } else {
                    Value::Nil
                };
                Err(EvalError::Return(val))
            }
        }
    }

    fn eval_expr(&mut self, expr: &Expr, env: Rc<RefCell<Environment>>) -> Result<Value, EvalError> {
        match expr {
            Expr::Literal(val) => Ok(val.clone()),
            Expr::Variable(name) => env.borrow().get(name).ok_or_else(|| format!("undefined variable `{name}`").into()),
            Expr::List(exprs) => {
                let mut values = Vec::new();
                for e in exprs {
                    values.push(self.eval_expr(e, env.clone())?);
                }
                Ok(Value::List(Rc::new(RefCell::new(values))))
            }
            Expr::Dict(pairs) => {
                let mut map = HashMap::new();
                for (k_expr, v_expr) in pairs {
                    let k = self.eval_expr(k_expr, env.clone())?.to_string();
                    let v = self.eval_expr(v_expr, env.clone())?;
                    map.insert(k, v);
                }
                Ok(Value::Dict(Rc::new(RefCell::new(map))))
            }
            Expr::Call { callee, args } => {
                let func = self.eval_expr(callee, env.clone())?;
                let mut values = Vec::new();
                for arg in args {
                    values.push(self.eval_expr(arg, env.clone())?);
                }
                match func {
                    Value::NativeFunction(f) => {
                        let is_print = if let Expr::Variable(name) = &**callee {
                            name == "print"
                        } else {
                            false
                        };
                        
                        if is_print {
                            let line = values.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" ");
                            self.output.push(line);
                            Ok(Value::Nil)
                        } else {
                            f(values).map_err(|e| e.into())
                        }
                    }
                    Value::Function(f) => self.call_function(f, values),
                    Value::Class(c) => {
                        let inst = Rc::new(RefCell::new(Instance {
                            class: c.clone(),
                            fields: HashMap::new(),
                        }));
                        if let Some(init) = c.methods.get("init") {
                            let mut bound_init = (**init).clone();
                            let method_env = Rc::new(RefCell::new(Environment::new(init.closure.clone())));
                            method_env.borrow_mut().define("self".to_string(), Value::Instance(inst.clone()));
                            bound_init.closure = Some(method_env);
                            self.call_function(Rc::new(bound_init), values)?;
                        }
                        Ok(Value::Instance(inst))
                    }
                    _ => Err(format!("can only call functions and classes, found {func:?}").into()),
                }
            }
            Expr::Get { object, name } => {
                let val = self.eval_expr(object, env)?;
                if let Value::Instance(inst) = val {
                    let inst_b = inst.borrow();
                    if let Some(v) = inst_b.fields.get(name) {
                        Ok(v.clone())
                    } else if let Some(m) = inst_b.class.methods.get(name) {
                        let mut bound = (**m).clone();
                        let method_env = Rc::new(RefCell::new(Environment::new(m.closure.clone())));
                        method_env.borrow_mut().define("self".to_string(), Value::Instance(inst.clone()));
                        bound.closure = Some(method_env);
                        Ok(Value::Function(Rc::new(bound)))
                    } else {
                        Err(format!("undefined property `{name}`").into())
                    }
                } else {
                    Err("only instances have properties".to_string().into())
                }
            }
            Expr::Set { object, name, value } => {
                let obj = self.eval_expr(object, env.clone())?;
                let val = self.eval_expr(value, env)?;
                if let Value::Instance(inst) = obj {
                    inst.borrow_mut().fields.insert(name.clone(), val.clone());
                    Ok(val)
                } else {
                    Err("only instances have properties".to_string().into())
                }
            }
            Expr::Index { target, index } => {
                let t = self.eval_expr(target, env.clone())?;
                let i = self.eval_expr(index, env)?;
                match t {
                    Value::List(l) => {
                        let idx = i.as_number().ok_or("index must be a number")? as usize;
                        l.borrow().get(idx).cloned().ok_or("index out of bounds".to_string().into())
                    }
                    Value::Dict(d) => {
                        Ok(d.borrow().get(&i.to_string()).cloned().unwrap_or(Value::Nil))
                    }
                    _ => Err("can only index lists and dictionaries".to_string().into()),
                }
            }
            Expr::SetIndex { target, index, value } => {
                let t = self.eval_expr(target, env.clone())?;
                let i = self.eval_expr(index, env.clone())?;
                let v = self.eval_expr(value, env)?;
                match t {
                    Value::List(l) => {
                        let idx = i.as_number().ok_or("index must be a number")? as usize;
                        let mut l = l.borrow_mut();
                        if idx < l.len() {
                            l[idx] = v.clone();
                            Ok(v)
                        } else {
                            Err("index out of bounds".to_string().into())
                        }
                    }
                    Value::Dict(d) => {
                        d.borrow_mut().insert(i.to_string(), v.clone());
                        Ok(v)
                    }
                    _ => Err("can only index lists and dictionaries".to_string().into()),
                }
            }
            Expr::Binary { left, op, right } => {
                let l = self.eval_expr(left, env.clone())?;
                let r = self.eval_expr(right, env)?;
                match op {
                    TokenKind::Plus => match (l, r) {
                        (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a + b)),
                        (a, b) => Ok(Value::String(format!("{a}{b}"))),
                    },
                    TokenKind::Minus => self.num_op(l, r, |a, b| a - b).map_err(|e| e.into()),
                    TokenKind::Star => self.num_op(l, r, |a, b| a * b).map_err(|e| e.into()),
                    TokenKind::Slash => self.num_op(l, r, |a, b| a / b).map_err(|e| e.into()),
                    TokenKind::EqualEqual => Ok(Value::Bool(l == r)),
                    TokenKind::BangEqual => Ok(Value::Bool(l != r)),
                    _ => Err("unsupported operator".to_string().into()),
                }
            }
        }
    }

    fn call_function(&mut self, f: Rc<UserFunction>, args: Vec<Value>) -> Result<Value, EvalError> {
        let call_env = Rc::new(RefCell::new(Environment::new(f.closure.clone())));
        for (i, param) in f.params.iter().enumerate() {
            call_env.borrow_mut().define(param.clone(), args.get(i).cloned().unwrap_or(Value::Nil));
        }
        for stmt in &f.body {
            match self.eval_stmt(stmt, call_env.clone()) {
                Ok(_) => {}
                Err(EvalError::Return(val)) => return Ok(val),
                Err(err) => return Err(err),
            }
        }
        Ok(Value::Nil)
    }

    fn is_truthy(val: &Value) -> bool {
        match val {
            Value::Nil => false,
            Value::Bool(b) => *b,
            _ => true,
        }
    }

    fn num_op(&self, l: Value, r: Value, f: fn(f64, f64) -> f64) -> Result<Value, String> {
        match (l, r) {
            (Value::Number(a), Value::Number(b)) => Ok(Value::Number(f(a, b))),
            _ => Err("operator expects numbers".to_string()),
        }
    }
}

impl Value {
    fn as_number(&self) -> Option<f64> {
        if let Value::Number(n) = self { Some(*n) } else { None }
    }
}

fn lex(source: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = source.chars().peekable();
    let mut line = 1;

    while let Some(ch) = chars.next() {
        match ch {
            ' ' | '\r' | '\t' => {}
            '\n' => { tokens.push(Token { kind: TokenKind::Newline, line }); line += 1; }
            '(' => tokens.push(Token { kind: TokenKind::LeftParen, line }),
            ')' => tokens.push(Token { kind: TokenKind::RightParen, line }),
            '[' => tokens.push(Token { kind: TokenKind::LeftBracket, line }),
            ']' => tokens.push(Token { kind: TokenKind::RightBracket, line }),
            '{' => tokens.push(Token { kind: TokenKind::LeftBrace, line }),
            '}' => tokens.push(Token { kind: TokenKind::RightBrace, line }),
            ',' => tokens.push(Token { kind: TokenKind::Comma, line }),
            '.' => tokens.push(Token { kind: TokenKind::Dot, line }),
            ':' => tokens.push(Token { kind: TokenKind::Colon, line }),
            '+' => {
                if let Some('+') = chars.peek() {
                    chars.next();
                    tokens.push(Token { kind: TokenKind::PlusPlus, line });
                } else if let Some('=') = chars.peek() {
                    chars.next();
                    tokens.push(Token { kind: TokenKind::PlusEqual, line });
                } else {
                    tokens.push(Token { kind: TokenKind::Plus, line });
                }
            }
            '-' => {
                if let Some('-') = chars.peek() {
                    let mut temp_chars = chars.clone();
                    temp_chars.next();
                    let next = temp_chars.peek();
                    if next.is_none() || next == Some(&' ') || next == Some(&'\t') || next == Some(&'\n') || next == Some(&'\r') {
                        while let Some(c) = chars.next() { if c == '\n' { line += 1; break; } }
                    } else {
                        chars.next();
                        tokens.push(Token { kind: TokenKind::MinusMinus, line });
                    }
                } else if let Some('=') = chars.peek() {
                    chars.next();
                    tokens.push(Token { kind: TokenKind::MinusEqual, line });
                } else {
                    tokens.push(Token { kind: TokenKind::Minus, line });
                }
            }
            '*' => {
                if let Some('=') = chars.peek() {
                    chars.next();
                    tokens.push(Token { kind: TokenKind::StarEqual, line });
                } else {
                    tokens.push(Token { kind: TokenKind::Star, line });
                }
            }
            '/' => {
                if let Some('/') = chars.peek() {
                    while let Some(c) = chars.next() { if c == '\n' { line += 1; break; } }
                } else if let Some('=') = chars.peek() {
                    chars.next();
                    tokens.push(Token { kind: TokenKind::SlashEqual, line });
                } else {
                    tokens.push(Token { kind: TokenKind::Slash, line });
                }
            }
            '=' => {
                if let Some('=') = chars.peek() {
                    chars.next();
                    tokens.push(Token { kind: TokenKind::EqualEqual, line });
                } else {
                    tokens.push(Token { kind: TokenKind::Equal, line });
                }
            }
            '!' => {
                if let Some('=') = chars.peek() {
                    chars.next();
                    tokens.push(Token { kind: TokenKind::EqualEqual, line });
                } else {
                    return Err(format!("unexpected character `!` at line {line}"));
                }
            }
            '"' | '\'' => {
                let mut s = String::new();
                while let Some(c) = chars.next() {
                    if c == ch { break; }
                    if c == '\\' {
                        match chars.next() {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some(esc) => s.push(esc),
                            None => return Err(format!("unterminated escape at line {line}")),
                        }
                    } else {
                        s.push(c);
                    }
                }
                tokens.push(Token { kind: TokenKind::String(s), line });
            }
            c if c.is_ascii_digit() => {
                let mut n = c.to_string();
                while let Some(nc) = chars.peek() {
                    if nc.is_ascii_digit() || *nc == '.' { n.push(chars.next().unwrap()); } else { break; }
                }
                tokens.push(Token { kind: TokenKind::Number(n.parse().unwrap()), line });
            }
            c if c.is_alphabetic() || c == '_' => {
                let mut s = c.to_string();
                while let Some(nc) = chars.peek() {
                    if nc.is_alphanumeric() || *nc == '_' { s.push(chars.next().unwrap()); } else { break; }
                }
                let kind = match s.as_str() {
                    "function" => TokenKind::Function,
                    "end" => TokenKind::End,
                    "then" => TokenKind::Then,
                    "do" => TokenKind::Do,
                    "class" => TokenKind::Class,
                    "if" => TokenKind::If,
                    "else" => TokenKind::Else,
                    "while" => TokenKind::While,
                    "return" => TokenKind::Return,
                    "true" => TokenKind::True,
                    "false" => TokenKind::False,
                    "self" => TokenKind::SelfKw,
                    _ => TokenKind::Ident(s),
                };
                tokens.push(Token { kind, line });
            }
            _ => return Err(format!("unexpected character `{ch}` at line {line}")),
        }
    }
    tokens.push(Token { kind: TokenKind::Eof, line });
    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self { Self { tokens, pos: 0 } }

    fn parse_program(&mut self) -> Result<Vec<Stmt>, String> {
        let mut stmts = Vec::new();
        while !self.is_at_end() {
            self.skip_newlines();
            if self.is_at_end() { break; }
            stmts.push(self.statement()?);
        }
        Ok(stmts)
    }

    fn statement(&mut self) -> Result<Stmt, String> {
        match &self.peek().kind {
            TokenKind::Function => self.fn_def(),
            TokenKind::Class => self.class_def(),
            TokenKind::If => self.if_stmt(),
            TokenKind::While => self.while_stmt(),
            TokenKind::Return => self.return_stmt(),
            _ => {
                let expr = self.expression()?;
                let kind = self.peek().kind.clone();
                match kind {
                    TokenKind::Equal | TokenKind::PlusEqual | TokenKind::MinusEqual | TokenKind::StarEqual | TokenKind::SlashEqual => {
                        self.advance();
                        let rhs = if kind == TokenKind::Equal {
                            self.expression()?
                        } else {
                            let bin_op = match kind {
                                TokenKind::PlusEqual => TokenKind::Plus,
                                TokenKind::MinusEqual => TokenKind::Minus,
                                TokenKind::StarEqual => TokenKind::Star,
                                TokenKind::SlashEqual => TokenKind::Slash,
                                _ => unreachable!(),
                            };
                            Expr::Binary {
                                left: Box::new(expr.clone()),
                                op: bin_op,
                                right: Box::new(self.expression()?),
                            }
                        };

                        match expr {
                            Expr::Variable(name) => Ok(Stmt::Assign { name, expr: rhs }),
                            Expr::Get { object, name } => Ok(Stmt::Expr(Expr::Set { object, name, value: Box::new(rhs) })),
                            Expr::Index { target, index } => Ok(Stmt::Expr(Expr::SetIndex { target, index, value: Box::new(rhs) })),
                            _ => Err(format!("invalid assignment target at line {}", self.peek().line)),
                        }
                    }
                    TokenKind::PlusPlus | TokenKind::MinusMinus => {
                        self.advance();
                        let bin_op = if kind == TokenKind::PlusPlus { TokenKind::Plus } else { TokenKind::Minus };
                        let rhs = Expr::Binary {
                            left: Box::new(expr.clone()),
                            op: bin_op,
                            right: Box::new(Expr::Literal(Value::Number(1.0))),
                        };
                        match expr {
                            Expr::Variable(name) => Ok(Stmt::Assign { name, expr: rhs }),
                            Expr::Get { object, name } => Ok(Stmt::Expr(Expr::Set { object, name, value: Box::new(rhs) })),
                            Expr::Index { target, index } => Ok(Stmt::Expr(Expr::SetIndex { target, index, value: Box::new(rhs) })),
                            _ => Err(format!("invalid assignment target at line {}", self.peek().line)),
                        }
                    }
                    _ => Ok(Stmt::Expr(expr)),
                }
            }
        }
    }

    fn fn_def(&mut self) -> Result<Stmt, String> {
        self.advance();
        let name = self.expect_ident("function name")?;
        self.expect(TokenKind::LeftParen, "'('")?;
        let mut params = Vec::new();
        if !self.check(TokenKind::RightParen) {
            loop {
                params.push(self.expect_ident("parameter name")?);
                if !self.match_token(TokenKind::Comma) { break; }
            }
        }
        if !self.match_token(TokenKind::RightParen) {
            let found = self.peek();
            return Err(format!("expected ')' at line {} but found {}\ndid you forget a comma?", found.line, format_token(&found.kind)));
        }
        let body = self.parse_block(&[TokenKind::End])?;
        self.expect(TokenKind::End, "end")?;
        Ok(Stmt::FnDef(Rc::new(UserFunction { name, params, body, closure: None })))
    }

    fn class_def(&mut self) -> Result<Stmt, String> {
        self.advance();
        let name = self.expect_ident("class name")?;
        let mut methods = HashMap::new();
        while !self.check(TokenKind::End) && !self.is_at_end() {
            self.skip_newlines();
            if self.check(TokenKind::End) { break; }
            if let Stmt::FnDef(func) = self.fn_def()? {
                methods.insert(func.name.clone(), func);
            } else {
                return Err(format!("expected function in class at line {}", self.peek().line));
            }
            self.skip_newlines();
        }
        self.expect(TokenKind::End, "end")?;
        Ok(Stmt::ClassDef(Rc::new(Class { name, methods })))
    }

    fn parse_block(&mut self, stop_tokens: &[TokenKind]) -> Result<Vec<Stmt>, String> {
        let mut stmts = Vec::new();
        while !self.is_at_end() {
            self.skip_newlines();
            if stop_tokens.iter().any(|t| self.check(t.clone())) {
                break;
            }
            stmts.push(self.statement()?);
            self.skip_newlines();
        }
        Ok(stmts)
    }

    fn if_stmt(&mut self) -> Result<Stmt, String> {
        self.advance();
        let condition = self.expression()?;
        self.expect(TokenKind::Then, "then")?;
        let then_branch = self.parse_block(&[TokenKind::Else, TokenKind::End])?;
        let mut else_branch = None;
        if self.match_token(TokenKind::Else) {
            else_branch = Some(self.parse_block(&[TokenKind::End])?);
        }
        self.expect(TokenKind::End, "end")?;
        Ok(Stmt::If { condition, then_branch, else_branch })
    }

    fn while_stmt(&mut self) -> Result<Stmt, String> {
        self.advance();
        let condition = self.expression()?;
        self.expect(TokenKind::Do, "do")?;
        let body = self.parse_block(&[TokenKind::End])?;
        self.expect(TokenKind::End, "end")?;
        Ok(Stmt::While { condition, body })
    }

    fn return_stmt(&mut self) -> Result<Stmt, String> {
        self.advance();
        let expr = if !self.check(TokenKind::Newline) && !self.is_at_end() && !self.check(TokenKind::End) {
            Some(self.expression()?)
        } else {
            None
        };
        Ok(Stmt::Return(expr))
    }

    fn expression(&mut self) -> Result<Expr, String> { self.equality() }

    fn equality(&mut self) -> Result<Expr, String> {
        let mut expr = self.term()?;
        while self.match_token(TokenKind::EqualEqual) || self.match_token(TokenKind::BangEqual) {
            let op = self.previous().kind.clone();
            let right = self.term()?;
            expr = Expr::Binary { left: Box::new(expr), op, right: Box::new(right) };
        }
        Ok(expr)
    }

    fn term(&mut self) -> Result<Expr, String> {
        let mut expr = self.factor()?;
        while self.match_token(TokenKind::Plus) || self.match_token(TokenKind::Minus) {
            let op = self.previous().kind.clone();
            let right = self.factor()?;
            expr = Expr::Binary { left: Box::new(expr), op, right: Box::new(right) };
        }
        Ok(expr)
    }

    fn factor(&mut self) -> Result<Expr, String> {
        let mut expr = self.call()?;
        while self.match_token(TokenKind::Star) || self.match_token(TokenKind::Slash) {
            let op = self.previous().kind.clone();
            let right = self.call()?;
            expr = Expr::Binary { left: Box::new(expr), op, right: Box::new(right) };
        }
        Ok(expr)
    }

    fn call(&mut self) -> Result<Expr, String> {
        let mut expr = self.primary()?;
        loop {
            if self.match_token(TokenKind::LeftParen) {
                let mut args = Vec::new();
                if !self.check(TokenKind::RightParen) {
                    loop {
                        args.push(self.expression()?);
                        if !self.match_token(TokenKind::Comma) { break; }
                    }
                }
                if !self.match_token(TokenKind::RightParen) {
                    let found = self.peek();
                    return Err(format!("expected ')' at line {} but found {}\ndid you forget a comma?", found.line, format_token(&found.kind)));
                }
                expr = Expr::Call { callee: Box::new(expr), args };
            } else if self.match_token(TokenKind::Dot) {
                let name = self.expect_ident("property name")?;
                expr = Expr::Get { object: Box::new(expr), name };
            } else if self.match_token(TokenKind::LeftBracket) {
                let index = self.expression()?;
                self.expect(TokenKind::RightBracket, "']'")?;
                expr = Expr::Index { target: Box::new(expr), index: Box::new(index) };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn primary(&mut self) -> Result<Expr, String> {
        match self.advance().kind.clone() {
            TokenKind::Number(n) => Ok(Expr::Literal(Value::Number(n))),
            TokenKind::String(s) => Ok(Expr::Literal(Value::String(s))),
            TokenKind::True => Ok(Expr::Literal(Value::Bool(true))),
            TokenKind::False => Ok(Expr::Literal(Value::Bool(false))),
            TokenKind::SelfKw => Ok(Expr::Variable("self".to_string())),
            TokenKind::Ident(s) => Ok(Expr::Variable(s)),
            TokenKind::LeftBracket => {
                let mut exprs = Vec::new();
                if !self.check(TokenKind::RightBracket) {
                    loop {
                        exprs.push(self.expression()?);
                        if !self.match_token(TokenKind::Comma) { break; }
                    }
                }
                if !self.match_token(TokenKind::RightBracket) {
                    let found = self.peek();
                    return Err(format!("expected ']' at line {} but found {}\ndid you forget a comma?", found.line, format_token(&found.kind)));
                }
                Ok(Expr::List(exprs))
            }
            TokenKind::LeftBrace => {
                let mut pairs = Vec::new();
                if !self.check(TokenKind::RightBrace) {
                    loop {
                        let k = self.expression()?;
                        if self.match_token(TokenKind::Colon) || self.match_token(TokenKind::Equal) {
                            pairs.push((k, self.expression()?));
                        } else {
                            return Err(format!("expected ':' or '=' in dictionary at line {} but found {}", self.peek().line, format_token(&self.peek().kind)));
                        }
                        if !self.match_token(TokenKind::Comma) { break; }
                    }
                }
                if !self.match_token(TokenKind::RightBrace) {
                    let found = self.peek();
                    return Err(format!("expected '}}' at line {} but found {}\ndid you forget a comma?", found.line, format_token(&found.kind)));
                }
                Ok(Expr::Dict(pairs))
            }
            TokenKind::LeftParen => {
                let expr = self.expression()?;
                self.expect(TokenKind::RightParen, "')'")?;
                Ok(expr)
            }
            k => Err(format!("expected expression at line {}, found {}", self.previous().line, format_token(&k))),
        }
    }

    fn skip_newlines(&mut self) { while self.match_token(TokenKind::Newline) {} }

    fn match_token(&mut self, kind: TokenKind) -> bool {
        if self.check(kind) { self.advance(); true } else { false }
    }

    fn check(&self, kind: TokenKind) -> bool {
        if self.is_at_end() { false } else { self.peek().kind == kind }
    }

    fn expect(&mut self, kind: TokenKind, msg: &str) -> Result<(), String> {
        if self.check(kind) { self.advance(); Ok(()) }
        else { Err(format!("expected {} at line {} but found {}", msg, self.peek().line, format_token(&self.peek().kind))) }
    }

    fn expect_ident(&mut self, msg: &str) -> Result<String, String> {
        if let TokenKind::Ident(s) = &self.peek().kind {
            let s = s.clone();
            self.advance();
            Ok(s)
        } else {
            Err(format!("expected {} at line {}, found {}", msg, self.peek().line, format_token(&self.peek().kind)))
        }
    }

    fn is_at_end(&self) -> bool { self.peek().kind == TokenKind::Eof }
    fn peek(&self) -> &Token { self.tokens.get(self.pos).unwrap() }
    fn previous(&self) -> &Token { self.tokens.get(self.pos - 1).unwrap() }
    fn advance(&mut self) -> &Token { if !self.is_at_end() { self.pos += 1; } self.previous() }
}
