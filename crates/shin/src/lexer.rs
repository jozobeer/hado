#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Def,
    Main,
    Eq,
    Plus,
    Int(i64),
}

pub fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let mut toks = Vec::new();
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '=' {
            toks.push(Tok::Eq);
            i += 1;
            continue;
        }
        if c == '+' {
            toks.push(Tok::Plus);
            i += 1;
            continue;
        }
        if c.is_ascii_digit() {
            let mut j = i;
            while j < bytes.len() && (bytes[j] as char).is_ascii_digit() {
                j += 1;
            }
            let lit: i64 = src[i..j]
                .parse()
                .map_err(|_| format!("integer out of range: {}", &src[i..j]))?;
            toks.push(Tok::Int(lit));
            i = j;
            continue;
        }
        if c.is_ascii_alphabetic() || c == '_' {
            let mut j = i;
            while j < bytes.len()
                && ((bytes[j] as char).is_ascii_alphanumeric() || bytes[j] == b'_')
            {
                j += 1;
            }
            match &src[i..j] {
                "def" => toks.push(Tok::Def),
                "main" => toks.push(Tok::Main),
                word => return Err(format!("unexpected word: {word}")),
            }
            i = j;
            continue;
        }
        return Err(format!("unexpected character: {c}"));
    }
    Ok(toks)
}
