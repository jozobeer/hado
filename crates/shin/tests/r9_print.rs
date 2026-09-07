use std::process::Command;

#[test]
fn generated_lists_print_source_not_constructor_chains() {
    let dir = std::env::temp_dir().join(format!("shin-r9-list-print-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut seed = 731_u64;
    for length in 0..12 {
        let values: Vec<_> = (0..length).map(|_| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((seed >> 32) as i32 % 1000).to_string()
        }).collect();
        let expected = format!("[{}]", values.join(", "));
        for (case, ty, expression, output) in [
            ("plain", "List<Int>", expected.clone(), expected.clone()),
            ("nested", "List<List<Int>>", format!("[{expected}, []]"), format!("[{expected}, []]")),
            ("option", "Option<List<Int>>", format!("Some({expected})"), format!("Some({expected})")),
        ] {
            let path = dir.join(format!("{case}-{length}.shin"));
            std::fs::write(&path, format!("def identity (x: {ty}): {ty} => x\ndef main = identity ({expression})")).unwrap();
            let built = Command::new(env!("CARGO_BIN_EXE_shin")).arg("build").arg(&path).output().unwrap();
            assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
            let actual = Command::new(path.with_extension("")).output().unwrap();
            assert_eq!(actual.status.code(), Some(0));
            assert_eq!(actual.stdout, format!("{output}\n").as_bytes());
            assert!(actual.stderr.is_empty());
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

fn literal(value: &str, quote: char) -> String {
    let mut result = quote.to_string();
    for ch in value.chars() {
        match ch {
            '\n' => result.push_str("\\n"),
            '\t' => result.push_str("\\t"),
            '\\' => result.push_str("\\\\"),
            ch if ch == quote => { result.push('\\'); result.push(ch); }
            ch => result.push(ch),
        }
    }
    result.push(quote);
    result
}

#[test]
fn nested_text_is_quoted_but_outer_result_payload_is_raw() {
    let dir = std::env::temp_dir().join(format!("shin-r9-text-print-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (index, ch) in ['\n', '\t', '\\', '\'', '"', 'あ', '😀'].into_iter().enumerate() {
        for (kind, ty, value, encoded) in [
            ("char", "Char", ch.to_string(), literal(&ch.to_string(), '\'')),
            ("string", "String", format!("item{index}{ch}\"\\\n\t'"), literal(&format!("item{index}{ch}\"\\\n\t'"), '"')),
        ] {
            for (case, source, expected, status) in [
                ("top", format!("def main = {encoded}"), value.clone(), 0),
                ("nested", format!("def main = Some({encoded})"), format!("Some({encoded})"), 0),
                ("list", format!("def main = [{encoded}]"), format!("[{encoded}]"), 0),
                ("ok", format!("def identity (x: Result<{ty}, Int>) => x\ndef main = identity (Ok({encoded}))"), value.clone(), 0),
                ("error", format!("def identity (x: Result<Int, {ty}>) => x\ndef main = identity (Error({encoded}))"), value, 1),
            ] {
                let path = dir.join(format!("{index}-{kind}-{case}"));
                shin::build::executable(&source, "text-print.shin", &path).unwrap();
                let output = Command::new(&path).output().unwrap();
                let expected = format!("{expected}\n");
                assert_eq!(output.status.code(), Some(status));
                assert_eq!(output.stdout, if status == 0 { expected.as_bytes() } else { b"" });
                assert_eq!(output.stderr, if status == 1 { expected.as_bytes() } else { b"" });
            }
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}
