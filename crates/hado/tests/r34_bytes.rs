use hado::check_source;
use hado::mono::Type;
use hado::types::Ty;

#[test]
fn bytes_machine_representation_is_a_pointer() {
    let checked = check_source("def main = 0", "t").unwrap();
    assert_eq!(
        hado::repr::machine_type(&Type::Bytes, &checked).unwrap(),
        hado::repr::MachineType::Pointer
    );
}

#[test]
fn bytes_annotation_resolves_and_length_is_int() {
    let checked = check_source(
        "def f (b: Bytes) => b\ndef main = Bytes.length (f (Bytes.of_string \"a\"))",
        "t",
    )
    .unwrap();
    assert_eq!(checked.main, Ty::Int);
}

#[test]
fn bytes_is_a_reserved_builtin_name() {
    assert_eq!(
        check_source("type Bytes = | A\ndef main = 1", "t")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E205")
    );
}

#[test]
fn bytes_length_rejects_string() {
    assert_eq!(
        check_source("def main = Bytes.length \"a\"", "t")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E305")
    );
}

#[test]
fn runtime_declarations_expose_bytes_symbols() {
    use hado::runtime_abi::{AbiType, RuntimeFunction};
    assert_eq!(
        RuntimeFunction::BytesLength.declaration().symbol,
        "hado_bytes_length"
    );
    assert_eq!(
        RuntimeFunction::BytesGet.declaration().symbol,
        "hado_bytes_get"
    );
    assert_eq!(
        RuntimeFunction::BytesIsUtf8.declaration().symbol,
        "hado_bytes_is_utf8"
    );
    assert_eq!(
        RuntimeFunction::BytesDisplay.declaration().symbol,
        "hado_bytes_display"
    );
    assert_eq!(
        RuntimeFunction::BytesOfString.declaration().symbol,
        "hado_bytes_of_string"
    );
    assert_eq!(
        RuntimeFunction::BytesToStringUnchecked.declaration().symbol,
        "hado_bytes_to_string_unchecked"
    );
    assert_eq!(
        RuntimeFunction::BytesAlloc.declaration().symbol,
        "hado_bytes_alloc"
    );
    assert_eq!(
        RuntimeFunction::BytesFillUnchecked.declaration().symbol,
        "hado_bytes_fill_unchecked"
    );
    assert_eq!(
        RuntimeFunction::BytesIsUtf8.declaration().result,
        Some(AbiType::I8)
    );
}

fn write_utf8_cases(path: &std::path::Path) {
    // 分類と独立 oracle を二重管理しないため、CLI 判定器の生成器を共有する。
    let generator = include_str!("../../../scripts/verify-r34.sh")
        .split_once("# BEGIN UTF8 CASE GENERATOR\n")
        .unwrap()
        .1
        .split_once("# END UTF8 CASE GENERATOR")
        .unwrap()
        .0;
    let output = std::process::Command::new("python3")
        .arg("-c")
        .arg(format!(
            "{generator}\n{}",
            r#"
cases = utf8_cases(random.Random(34))
assert cases and any(valid for _, _, valid in cases) and any(not valid for _, _, valid in cases)
print("typedef struct { size_t length; unsigned char bytes[%d]; } Utf8Probe;" %
      max(len(seq) for _, seq, _ in cases))
print("static const struct { Utf8Probe value; int accepted; const char *name; } utf8_cases[] = {")
for name, seq, accepted in cases:
    print('  {{%d, {%s}}, %d, "%s"},' %
          (len(seq), ", ".join(map(str, seq)) or "0", accepted, name))
print("};")
"#
        ))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output.stdout.is_empty());
    std::fs::write(path, output.stdout).unwrap();
}

#[test]
fn bytes_is_utf8_and_display_at_the_c_level() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir().join(format!("hado-bytes-runtime-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    write_utf8_cases(&dir.join("utf8_cases.h"));
    let source = dir.join("test.c");
    let object = dir.join("test.o");
    let executable = dir.join("test");
    std::fs::write(
        &source,
        r#"
#include "hado_runtime.h"
#include <assert.h>
#include <string.h>
#include <stdio.h>
#include "utf8_cases.h"

static void assert_utf8_round_trip(const Utf8Probe *probe) {
    const HadoString *original = (const HadoString *)probe;
    assert(hado_bytes_is_utf8(original) == 1);
    HadoString *result = hado_bytes_of_string(hado_bytes_to_string_unchecked(original));
    assert(result->length == probe->length);
    assert(memcmp(result->bytes, probe->bytes, probe->length) == 0);
}

int32_t hado_entry(void) {
    struct { size_t length; unsigned char bytes[3]; } valid = {3, {0xe3, 0x81, 0x82}};
    struct { size_t length; unsigned char bytes[2]; } two = {2, {72, 105}};
    struct { size_t length; } empty = {0};
    const HadoString *valid_string = (const HadoString *)&valid;
    const HadoString *empty_string = (const HadoString *)&empty;
    assert(hado_bytes_is_utf8(valid_string) == 1);
    // 長さの外にも正しい継続バイトを置き、境界検査の削除が偶然のメモリ内容で隠れないようにする。
    for (size_t length = 1; length < valid.length; ++length) {
        struct { size_t length; unsigned char bytes[3]; } truncated = {length, {0xe3, 0x81, 0x82}};
        assert(hado_bytes_is_utf8((const HadoString *)&truncated) == 0);
    }
    assert(hado_bytes_is_utf8((const HadoString *)&two) == 1);
    assert(hado_bytes_is_utf8(empty_string) == 1);
    for (size_t index = 0; index < sizeof(utf8_cases) / sizeof(utf8_cases[0]); ++index) {
        const Utf8Probe *probe = &utf8_cases[index].value;
        int actual = hado_bytes_is_utf8((const HadoString *)probe);
        if (actual != utf8_cases[index].accepted) {
            fprintf(stderr, "UTF-8 %s: expected=%d actual=%d\n",
                    utf8_cases[index].name, utf8_cases[index].accepted, actual);
        }
        assert(actual == utf8_cases[index].accepted);
        if (utf8_cases[index].accepted) assert_utf8_round_trip(probe);
    }
    assert(hado_bytes_length((const HadoString *)&two) == 2);
    assert(hado_bytes_get((const HadoString *)&two, 1) == 105);
    HadoString *displayed = hado_bytes_display((const HadoString *)&two);
    assert(displayed->length == strlen("Bytes.of_list [72, 105]"));
    assert(memcmp(displayed->bytes, "Bytes.of_list [72, 105]", displayed->length) == 0);
    HadoString *empty_display = hado_bytes_display(empty_string);
    assert(empty_display->length == strlen("Bytes.of_list []"));
    assert(memcmp(empty_display->bytes, "Bytes.of_list []", empty_display->length) == 0);
    return 0;
}
"#,
    )
    .unwrap();
    let status = std::process::Command::new("cc")
        .args(["-std=c11", "-Wall", "-Werror", "-c"])
        .arg(&source)
        .arg("-I")
        .arg(root.join("runtime"))
        .arg("-o")
        .arg(&object)
        .status()
        .unwrap();
    assert!(status.success());
    hado::link_executable(&object, &executable).unwrap();
    assert!(std::process::Command::new(executable)
        .status()
        .unwrap()
        .success());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn string_intrinsics_use_scalar_indices_at_the_c_level() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir().join(format!("hado-string-operations-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("test.c");
    std::fs::write(&source, string_operations_probe()).unwrap();
    let object = dir.join("test.o");
    let status = std::process::Command::new("cc")
        .args(["-std=c11", "-Wall", "-Werror", "-c"])
        .arg(&source)
        .arg("-I")
        .arg(root.join("runtime"))
        .arg("-o")
        .arg(&object)
        .status()
        .unwrap();
    assert!(status.success());
    let executable = dir.join("test");
    hado::link_executable(&object, &executable).unwrap();
    assert!(std::process::Command::new(executable)
        .status()
        .unwrap()
        .success());
    std::fs::remove_dir_all(dir).unwrap();
}

fn string_operations_probe() -> String {
    let mut code = String::from("#include \"hado_runtime.h\"\n#include <assert.h>\n#include <string.h>\nint32_t hado_entry(void) {\n");
    for text in ["", "ababa", "éあ😀あ", "a\0éあ😀\0a"] {
        code.push_str("{\n");
        code.push_str(&c_string_value("value", text));
        let boundaries: Vec<_> = text
            .char_indices()
            .map(|(offset, _)| offset)
            .chain(std::iter::once(text.len()))
            .collect();
        for (index, scalar) in text.chars().enumerate() {
            code.push_str(&format!(
                "assert(hado_string_get(value, {index}) == {});\n",
                scalar as u32
            ));
        }
        for start in 0..boundaries.len() {
            for end in start..boundaries.len() {
                code.push_str("{\n");
                code.push_str(&c_string_value(
                    "expected",
                    &text[boundaries[start]..boundaries[end]],
                ));
                code.push_str(&format!(
                    "HadoString *actual = hado_string_slice(value, {start}, {end});\n"
                ));
                code.push_str("assert(actual != value);\nassert(actual->length == expected->length);\nassert(memcmp(actual->bytes, expected->bytes, expected->length) == 0);\n}\n");
            }
        }
        code.push_str(&string_find_assertions(text, &boundaries));
        code.push_str("}\n");
    }
    code.push_str("return 0;\n}\n");
    code
}

fn c_string_value(name: &str, text: &str) -> String {
    let bytes = text
        .bytes()
        .map(|byte| byte.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!("struct {{ size_t length; unsigned char bytes[{}]; }} {name}_storage = {{{}, {{{}}}}};\nconst HadoString *{name} = (const HadoString *)&{name}_storage;\n",
        text.len().max(1), text.len(), if text.is_empty() { "0" } else { &bytes })
}

fn string_find_assertions(text: &str, boundaries: &[usize]) -> String {
    let mut code = String::new();
    for separator in [
        "a",
        "aba",
        "é",
        "あ",
        "😀",
        "あ😀",
        "\0",
        "\0a",
        "Z",
        "longer than the input",
    ] {
        code.push_str("{\n");
        code.push_str(&c_string_value("separator", separator));
        for (from, byte) in boundaries.iter().enumerate() {
            let expected = text[*byte..]
                .find(separator)
                .map(|offset| text[..byte + offset].chars().count() as i64)
                .unwrap_or(-1);
            code.push_str(&format!(
                "assert(hado_string_find(value, separator, {from}) == {expected});\n"
            ));
        }
        code.push_str("}\n");
    }
    code
}
