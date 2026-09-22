use std::process::Command;

#[test]
fn generated_lists_print_source_not_constructor_chains() {
    let dir = std::env::temp_dir().join(format!("hado-r9-list-print-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut seed = 731_u64;
    for length in [0, 1, 3] {
        let values: Vec<_> = (0..length)
            .map(|_| {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                ((seed >> 32) as i32 % 1000).to_string()
            })
            .collect();
        let expected = format!("[{}]", values.join(", "));
        for (case, ty, expression, output) in [
            ("plain", "Int List", expected.clone(), expected.clone()),
            (
                "nested",
                "Int List List",
                format!("[{expected}, []]"),
                format!("[{expected}, []]"),
            ),
            (
                "option",
                "Int List Option",
                format!("Some({expected})"),
                format!("Some({expected})"),
            ),
        ] {
            let case_dir = dir.join(format!("{case}-{length}"));
            std::fs::create_dir_all(&case_dir).unwrap();
            let path = case_dir.join("main.hd");
            std::fs::write(
                &path,
                format!("def identity (x: {ty}): {ty} => x\ndef main = identity ({expression})"),
            )
            .unwrap();
            let built = Command::new(env!("CARGO_BIN_EXE_hado"))
                .arg("build")
                .arg(&path)
                .output()
                .unwrap();
            assert!(
                built.status.success(),
                "{}",
                String::from_utf8_lossy(&built.stderr)
            );
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
            ch if ch == quote => {
                result.push('\\');
                result.push(ch);
            }
            ch => result.push(ch),
        }
    }
    result.push(quote);
    result
}

#[test]
fn nested_text_is_quoted_but_outer_result_payload_is_raw() {
    let dir = std::env::temp_dir().join(format!("hado-r9-text-print-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let values: Vec<(String, String, String, String)> = ['\n', '\t', '\\', '\'', '"', 'あ', '😀']
        .into_iter()
        .enumerate()
        .map(|(index, ch)| {
            let char_value = ch.to_string();
            let string_value = format!("item{index}{ch}\"\\\n\t'");
            (
                literal(&char_value, '\''),
                char_value,
                literal(&string_value, '"'),
                string_value,
            )
        })
        .collect();
    // The outermost payload stays raw: one program per character and kind.
    for (index, (char_encoded, char_value, string_encoded, string_value)) in
        values.iter().enumerate()
    {
        for (kind, encoded, expected) in [
            ("char", char_encoded.as_str(), char_value.as_str()),
            ("string", string_encoded.as_str(), string_value.as_str()),
        ] {
            let path = dir.join(format!("{index}-{kind}-top"));
            hado::build::executable(&format!("def main = {encoded}"), "text-print.hd", &path)
                .unwrap();
            let output = Command::new(&path).output().unwrap();
            assert_eq!(output.status.code(), Some(0));
            assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
            assert!(output.stderr.is_empty());
        }
    }
    // Nested quoting (list element and constructor payload) in one program per form.
    for (kind, encoded) in [
        (
            "char",
            values
                .iter()
                .map(|value| value.0.clone())
                .collect::<Vec<_>>(),
        ),
        (
            "string",
            values
                .iter()
                .map(|value| value.2.clone())
                .collect::<Vec<_>>(),
        ),
    ] {
        for (form, nested) in [
            ("list", encoded.join(", ")),
            (
                "some",
                encoded
                    .iter()
                    .map(|item| format!("Some({item})"))
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
        ] {
            let path = dir.join(format!("{kind}-{form}"));
            hado::build::executable(&format!("def main = [{nested}]"), "text-print.hd", &path)
                .unwrap();
            let output = Command::new(&path).output().unwrap();
            assert_eq!(output.status.code(), Some(0));
            assert_eq!(output.stdout, format!("[{nested}]\n").as_bytes());
            assert!(output.stderr.is_empty());
        }
    }
    // A raised message stays raw on stderr: all characters in one program per kind.
    for (kind, raw) in [
        (
            "char",
            values
                .iter()
                .map(|value| value.1.as_str())
                .collect::<Vec<_>>()
                .join(""),
        ),
        (
            "string",
            values
                .iter()
                .map(|value| value.3.as_str())
                .collect::<Vec<_>>()
                .join(""),
        ),
    ] {
        let path = dir.join(format!("{kind}-raise"));
        hado::build::executable(
            &format!("def main = raise {{ message = {} }}", literal(&raw, '"')),
            "text-print.hd",
            &path,
        )
        .unwrap();
        let output = Command::new(&path).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, format!("{raw}\n").as_bytes());
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn floats_print_shortest_fixed_notation_and_special_values() {
    let dir = std::env::temp_dir().join(format!("hado-r9-float-print-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (index, (source, expected)) in [
        ("3.0".to_owned(), "3.0".to_owned()),
        ("0.1".to_owned(), "0.1".to_owned()),
        ("-0.0".to_owned(), "-0.0".to_owned()),
        (
            format!("{}.0", "1".to_owned() + &"0".repeat(300)),
            format!("{}.0", "1".to_owned() + &"0".repeat(300)),
        ),
        (
            format!("0.{}1", "0".repeat(299)),
            format!("0.{}1", "0".repeat(299)),
        ),
        ("0.0 / 0.0".to_owned(), "nan".to_owned()),
        ("1.0 / 0.0".to_owned(), "inf".to_owned()),
        ("-1.0 / 0.0".to_owned(), "-inf".to_owned()),
    ]
    .into_iter()
    .enumerate()
    {
        for (case, source, expected) in [
            ("top", format!("def main = {source}"), expected.clone()),
            (
                "nested",
                format!("def main = Some({source})"),
                format!("Some({expected})"),
            ),
        ] {
            let path = dir.join(format!("{index}-{case}"));
            hado::build::executable(&source, "float-print.hd", &path).unwrap();
            let actual = Command::new(&path).output().unwrap();
            assert_eq!(actual.status.code(), Some(0));
            assert_eq!(actual.stdout, format!("{expected}\n").as_bytes());
            assert!(actual.stderr.is_empty());
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn generated_finite_floats_match_an_independent_decimal_oracle() {
    let oracle = Command::new("python3").args(["-c", r#"
import struct, decimal, math
state = 92741
patterns = [1, (1 << 63) | 1, 0x7fefffffffffffff, 0xffefffffffffffff, 0x0010000000000000, 0x000fffffffffffff]
for _ in range(128):
    state = (state * 6364136223846793005 + 1) % (1 << 64)
    patterns.append(state)
for bits in patterns:
    value = struct.unpack('>d', bits.to_bytes(8, 'big'))[0]
    if not math.isfinite(value):
        continue
    text = format(decimal.Decimal(repr(value)), 'f')
    print(text if '.' in text else text + '.0')
"#]).output().unwrap();
    assert!(
        oracle.status.success(),
        "{}",
        String::from_utf8_lossy(&oracle.stderr)
    );
    let values = String::from_utf8(oracle.stdout).unwrap();
    assert!(values.lines().count() >= 128);
    let expected = format!("[{}]", values.lines().collect::<Vec<_>>().join(", "));
    let path = std::env::temp_dir().join(format!("hado-r9-generated-float-{}", std::process::id()));
    hado::build::executable(
        &format!("def main = {expected}"),
        "generated-float.hd",
        &path,
    )
    .unwrap();
    let output = Command::new(&path).output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
    assert!(output.stderr.is_empty());
    std::fs::remove_file(path).unwrap();
}
