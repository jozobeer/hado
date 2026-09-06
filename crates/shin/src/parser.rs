use crate::lexer::Tok;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Int(i64),
    Add(Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub body: Expr,
}

pub fn parse(toks: &[Tok]) -> Result<Program, String> {
    let mut pos = 0;
    let expect = |pos: &mut usize, want: &Tok| -> Result<(), String> {
        match toks.get(*pos) {
            Some(t) if t == want => {
                *pos += 1;
                Ok(())
            }
            other => Err(format!("expected {want:?}, found {other:?}")),
        }
    };
    expect(&mut pos, &Tok::Def)?;
    expect(&mut pos, &Tok::Main)?;
    expect(&mut pos, &Tok::Eq)?;
    let body = parse_expr(toks, &mut pos)?;
    if pos != toks.len() {
        return Err(format!("trailing tokens: {:?}", &toks[pos..]));
    }
    Ok(Program { body })
}

fn parse_expr(toks: &[Tok], pos: &mut usize) -> Result<Expr, String> {
    let mut left = parse_term(toks, pos)?;
    while *pos < toks.len() && toks[*pos] == Tok::Plus {
        *pos += 1;
        let right = parse_term(toks, pos)?;
        left = Expr::Add(Box::new(left), Box::new(right));
    }
    Ok(left)
}

fn parse_term(toks: &[Tok], pos: &mut usize) -> Result<Expr, String> {
    match toks.get(*pos) {
        Some(Tok::Int(n)) => {
            let n = *n;
            *pos += 1;
            Ok(Expr::Int(n))
        }
        other => Err(format!("expected integer, found {other:?}")),
    }
}
