use shin::{check_source, types::Ty};
use std::process::Command;

struct Generator(u64);

impl Generator {
    fn below(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 32) % n
    }

    fn expression(&mut self, ty: &Ty, depth: u32) -> String {
        if depth == 0 {
            return self.literal(ty);
        }
        match ty {
            Ty::Int | Ty::Float => {
                if self.below(4) == 0 {
                    return format!("(-{})", self.expression(ty, depth - 1));
                }
                let op = ["+", "-", "*"][self.below(3) as usize];
                self.binary(ty, op, depth)
            }
            Ty::Bool => self.boolean(depth),
            _ => self.literal(ty),
        }
    }

    fn literal(&mut self, ty: &Ty) -> String {
        let value = self.below(1000);
        match ty {
            Ty::Int => value.to_string(),
            Ty::Float => format!("{value}.25"),
            Ty::Bool => (value % 2 == 0).to_string(),
            Ty::Char => format!("'{}'", (b'a' + (value % 26) as u8) as char),
            Ty::String => format!("\"value{value}\""),
            Ty::Unit => "()".into(),
            _ => unreachable!(),
        }
    }

    fn binary(&mut self, ty: &Ty, op: &str, depth: u32) -> String {
        let left = self.expression(ty, depth - 1);
        let right = self.expression(ty, depth - 1);
        format!("({left} {op} {right})")
    }

    fn boolean(&mut self, depth: u32) -> String {
        match self.below(4) {
            0 => format!("(not {})", self.expression(&Ty::Bool, depth - 1)),
            1 => {
                let op = ["&&", "||"][self.below(2) as usize];
                self.binary(&Ty::Bool, op, depth)
            }
            2 => {
                let ty = [Ty::Int, Ty::Float][self.below(2) as usize].clone();
                let op = ["<", "<=", ">", ">="][self.below(4) as usize];
                self.binary(&ty, op, depth)
            }
            _ => {
                let ty = scalar_types()[self.below(6) as usize].clone();
                let op = ["==", "!="][self.below(2) as usize];
                self.binary(&ty, op, depth)
            }
        }
    }
}

fn scalar_types() -> [Ty; 6] {
    [Ty::Int, Ty::Float, Ty::Bool, Ty::Char, Ty::String, Ty::Unit]
}

#[test]
fn generated_type_round_trip_twenty() {
    let mut generator = Generator(0x543E_5EED);
    let path = std::env::temp_dir().join(format!("shin-round-{}.shin", std::process::id()));
    for index in 0..20 {
        let expected = scalar_types()[index % 6].clone();
        let source = format!("def main = {}", generator.expression(&expected, 3));
        assert_eq!(
            check_source(&source, "generated").unwrap().main,
            expected,
            "{source}"
        );
        std::fs::write(&path, &source).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_shin"))
            .args(["check", "--print-type"])
            .arg(&path)
            .output()
            .unwrap();
        assert!(output.status.success(), "{source}: {:?}", output);
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
        println!("type_round_ok: {index} {expected}");
    }
    std::fs::remove_file(path).unwrap();
    println!("type_round_n: 20");
}

#[test]
fn generated_mismatches_are_rejected() {
    let mut generator = Generator(0x543E_5EED);
    for _ in 0..20 {
        let left = generator.expression(&Ty::Int, 3);
        let right = generator.expression(&Ty::Bool, 3);
        let source = format!("def main = ({left} + {right})");
        let diagnostic = check_source(&source, "generated")
            .err()
            .expect("mismatched operands accepted");
        assert_eq!(diagnostic.code.as_deref(), Some("E305"), "{source}");
    }
}
