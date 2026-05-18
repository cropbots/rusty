import re

with open("khoron/src/lib.rs", "r") as f:
    src = f.read()

# 1. Add Repeat to TokenKind
src = src.replace("Return,\n    True", "Return,\n    Repeat,\n    True")
src = src.replace("'return'\".to_string(),\n        TokenKind::True", "'return'\".to_string(),\n        TokenKind::Repeat => \"'repeat'\".to_string(),\n        TokenKind::True")
src = src.replace("\"return\" => TokenKind::Return,", "\"return\" => TokenKind::Return,\n                    \"repeat\" => TokenKind::Repeat,")

# 2. Add LineStmt and change Stmt usage
line_stmt = """
#[derive(Debug, Clone, PartialEq)]
pub struct LineStmt {
    pub stmt: Stmt,
    pub line: usize,
}
"""
src = src.replace("pub enum Stmt {", line_stmt + "\npub enum Stmt {")
src = src.replace("pub body: Vec<Stmt>,", "pub body: Vec<LineStmt>,")
src = src.replace("then_branch: Vec<Stmt>,", "then_branch: Vec<LineStmt>,")
src = src.replace("else_branch: Option<Vec<Stmt>>,", "else_branch: Option<Vec<LineStmt>>,")
src = src.replace("body: Vec<Stmt>,", "body: Vec<LineStmt>,")
src = src.replace("Return(Option<Expr>),", "Return(Option<Expr>),\n    Repeat {\n        count: Expr,\n        body: Vec<LineStmt>,\n    },")

src = src.replace("fn parse_program(&mut self) -> Result<Vec<Stmt>, String>", "fn parse_program(&mut self) -> Result<Vec<LineStmt>, String>")
src = src.replace("let mut stmts = Vec::new();", "let mut stmts: Vec<LineStmt> = Vec::new();")
src = src.replace("fn statement(&mut self) -> Result<Stmt, String> {", "fn statement(&mut self) -> Result<LineStmt, String> {\n        let line = self.peek().line;")
src = src.replace("Ok(Stmt::", "Ok(LineStmt { line, stmt: Stmt::")
src = src.replace("Ok(LineStmt { line, stmt: Stmt::Assign", "Ok(LineStmt { line, stmt: Stmt::Assign") # it handles Ok(Stmt::...) -> Ok(LineStmt { line, stmt: Stmt::... })
src = src.replace("Ok(Stmt::Expr", "Ok(LineStmt { line, stmt: Stmt::Expr")
src = src.replace("Ok(Stmt::FnDef", "Ok(LineStmt { line, stmt: Stmt::FnDef")
src = src.replace("Ok(Stmt::ClassDef", "Ok(LineStmt { line, stmt: Stmt::ClassDef")
src = src.replace("Ok(Stmt::If", "Ok(LineStmt { line, stmt: Stmt::If")
src = src.replace("Ok(Stmt::While", "Ok(LineStmt { line, stmt: Stmt::While")
src = src.replace("Ok(Stmt::Return", "Ok(LineStmt { line, stmt: Stmt::Return")
src = src.replace("fn fn_def(&mut self) -> Result<Stmt, String>", "fn fn_def(&mut self) -> Result<Stmt, String>")

# wait, statement calls fn_def, class_def, if_stmt, etc.
# let's change those to return Stmt, and statement wraps them!
src = src.replace("TokenKind::Function => self.fn_def(),", "TokenKind::Function => Ok(LineStmt { line, stmt: self.fn_def()? }),")
src = src.replace("TokenKind::Class => self.class_def(),", "TokenKind::Class => Ok(LineStmt { line, stmt: self.class_def()? }),")
src = src.replace("TokenKind::If => self.if_stmt(),", "TokenKind::If => Ok(LineStmt { line, stmt: self.if_stmt()? }),")
src = src.replace("TokenKind::While => self.while_stmt(),", "TokenKind::While => Ok(LineStmt { line, stmt: self.while_stmt()? }),")
src = src.replace("TokenKind::Return => self.return_stmt(),", "TokenKind::Return => Ok(LineStmt { line, stmt: self.return_stmt()? }),")
# Repeat parsing
src = src.replace("TokenKind::While => Ok(LineStmt { line, stmt: self.while_stmt()? }),", "TokenKind::While => Ok(LineStmt { line, stmt: self.while_stmt()? }),\n            TokenKind::Repeat => Ok(LineStmt { line, stmt: self.repeat_stmt()? }),")

# add repeat_stmt function
repeat_fn = """
    fn repeat_stmt(&mut self) -> Result<Stmt, String> {
        self.advance();
        let count = self.expression()?;
        self.expect(TokenKind::Do, "do")?;
        let body = self.parse_block(&[TokenKind::End])?;
        self.expect(TokenKind::End, "end")?;
        Ok(Stmt::Repeat { count, body })
    }
"""
src = src.replace("fn return_stmt(&mut self)", repeat_fn + "\n    fn return_stmt(&mut self)")

src = src.replace("fn parse_block(&mut self, stop_tokens: &[TokenKind]) -> Result<Vec<Stmt>, String>", "fn parse_block(&mut self, stop_tokens: &[TokenKind]) -> Result<Vec<LineStmt>, String>")

# fix Stmt::FnDef etc that already had Ok(Stmt::...)
src = src.replace("Ok(LineStmt { line, stmt: Stmt::FnDef", "Ok(Stmt::FnDef")
src = src.replace("Ok(LineStmt { line, stmt: Stmt::ClassDef", "Ok(Stmt::ClassDef")
src = src.replace("Ok(LineStmt { line, stmt: Stmt::If", "Ok(Stmt::If")
src = src.replace("Ok(LineStmt { line, stmt: Stmt::While", "Ok(Stmt::While")
src = src.replace("Ok(LineStmt { line, stmt: Stmt::Return", "Ok(Stmt::Return")

# but what about Stmt::Assign and Stmt::Expr inside `statement()` ?
# we need them to be wrapped.
src = src.replace("Ok(Stmt::Assign", "Ok(LineStmt { line, stmt: Stmt::Assign")
src = src.replace("Ok(Stmt::Expr", "Ok(LineStmt { line, stmt: Stmt::Expr")
# wait, wait! inside if/while statements I did replace earlier. I will just undo those specific ones
src = src.replace("Ok(LineStmt { line, stmt: Stmt::Expr(Expr::Set", "Ok(LineStmt { line, stmt: Stmt::Expr(Expr::Set") # this is fine.

# 3. Add on_step and instruction limit to Runtime
src = src.replace("pub struct Runtime {", "pub struct Runtime<'a> {\n    pub on_step: Option<Box<dyn FnMut(usize) + 'a>>,\n    pub inst_count: usize,")
src = src.replace("impl Default for Runtime {", "impl<'a> Default for Runtime<'a> {")
src = src.replace("impl Runtime {", "impl<'a> Runtime<'a> {")
src = src.replace("fn default() -> Self {", "fn default() -> Self {")
src = src.replace("output: Vec::new(),", "output: Vec::new(),\n            on_step: None,\n            inst_count: 0,")

# 4. Modify eval_stmt to take LineStmt and handle limits
src = src.replace("fn eval_stmt(\n        &mut self,\n        stmt: &Stmt,", "fn eval_stmt(\n        &mut self,\n        line_stmt: &LineStmt,")
eval_prologue = """
        self.inst_count += 1;
        if self.inst_count > 10000 {
            return Err("execution limit exceeded".into());
        }
        if let Some(cb) = &mut self.on_step {
            cb(line_stmt.line);
        }
        match &line_stmt.stmt {
"""
src = src.replace("match stmt {", eval_prologue)
src = src.replace("Stmt::Assign", "Stmt::Assign")
src = src.replace("Stmt::Repeat", "Stmt::Repeat")

# Add evaluation for Repeat
repeat_eval = """
            Stmt::Repeat { count, body } => {
                let count_val = self.eval_expr(count, env.clone())?;
                if let Some(n) = count_val.as_number() {
                    let mut i = 0;
                    while i < n as i64 {
                        for s in body {
                            self.eval_stmt(s, env.clone())?;
                        }
                        i += 1;
                    }
                    Ok(Value::Nil)
                } else {
                    Err("repeat count must be a number".into())
                }
            }
"""
src = src.replace("Stmt::Return(expr) => {", repeat_eval + "\n            Stmt::Return(expr) => {")

# 5. Fix implicit call for no-arg function
implicit_call = """
            Stmt::Expr(expr) => {
                let val = self.eval_expr(expr, env.clone())?;
                match val {
                    Value::Function(f) => self.call_function(f, vec![]),
                    Value::NativeFunction(f) => f(vec![]).map_err(|e| e.into()),
                    _ => Ok(val),
                }
            }
"""
src = src.replace("Stmt::Expr(expr) => self.eval_expr(expr, env),", implicit_call)

with open("khoron/src/lib.rs", "w") as f:
    f.write(src)

print("done")
