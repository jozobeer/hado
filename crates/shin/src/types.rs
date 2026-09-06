use crate::parser::{Expr, Program};

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    Int,
}

/// M0: every literal is `Int` (`i64`), `+` keeps `Int`.
pub fn check(prog: &Program) -> Result<Ty, String> {
    check_expr(&prog.body)
}

fn check_expr(expr: &Expr) -> Result<Ty, String> {
    match expr {
        Expr::Int(_) => Ok(Ty::Int),
        Expr::Add(l, r) => {
            let lt = check_expr(l)?;
            let rt = check_expr(r)?;
            if lt == Ty::Int && rt == Ty::Int {
                Ok(Ty::Int)
            } else {
                Err(format!("+ needs Int, found {lt:?} and {rt:?}"))
            }
        }
    }
}
