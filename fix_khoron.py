import re

with open("khoron/src/lib.rs", "r") as f:
    src = f.read()

# Fix mismatched closing delimiters
src = src.replace("Ok(LineStmt { line, stmt: Stmt::Assign { name, expr: rhs }),", "Ok(LineStmt { line, stmt: Stmt::Assign { name, expr: rhs } }),")

src = src.replace("})),", "} })),")

src = src.replace("Ok(LineStmt { line, stmt: Stmt::Expr(expr)),", "Ok(LineStmt { line, stmt: Stmt::Expr(expr) }),")

with open("khoron/src/lib.rs", "w") as f:
    f.write(src)

print("fixed")
