#[test]
fn int_parse_checks_grammar_range_and_length_delimited_bytes() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir().join(format!("hado-int-runtime-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("test.c");
    let object = dir.join("test.o");
    let executable = dir.join("test");
    std::fs::write(
        &source,
        r#"
#include "hado_runtime.h"
#include <assert.h>
#include <string.h>

static const HadoString *string(const char *text) {
    size_t length = strlen(text);
    HadoString *value = hado_alloc(sizeof(HadoString) + length);
    value->length = length;
    memcpy(value->bytes, text, length);
    return value;
}

static void accepted(const char *text, int64_t expected) {
    const HadoString *value = string(text);
    assert(hado_int_parse_is_valid(value) == 1);
    assert(hado_int_parse_unchecked(value) == expected);
}

int32_t hado_entry(void) {
    accepted("0", 0);
    accepted("-0", 0);
    accepted("007", 7);
    accepted("-007", -7);
    accepted("9223372036854775807", INT64_MAX);
    accepted("-9223372036854775808", INT64_MIN);
    accepted("00000000000000000000000005", 5);
    accepted("00000000000000000000000009223372036854775807", INT64_MAX);
    const char *invalid[] = {
        "", "-", "+42", "--5", " 42", "42 ", "4 2", "1_000", "4.2",
        "４２", "4あ2", "a42", "4a2", "42a", "9223372036854775808",
        "-9223372036854775809", "18446744073709551616", "10000000000000000000",
        "100000000000000000000", "9999999999999999999999999999999999999",
        "00000000000000000000000009223372036854775808"
    };
    for (size_t i = 0; i < sizeof(invalid) / sizeof(invalid[0]); ++i) {
        assert(hado_int_parse_is_valid(string(invalid[i])) == 0);
    }
    struct { size_t length; unsigned char bytes[4]; } tail = {2, {'4', '2', '9', 'a'}};
    assert(hado_int_parse_is_valid((const HadoString *)&tail) == 1);
    assert(hado_int_parse_unchecked((const HadoString *)&tail) == 42);
    struct { size_t length; unsigned char bytes[3]; } nul = {3, {'4', 0, '2'}};
    assert(hado_int_parse_is_valid((const HadoString *)&nul) == 0);
    struct { size_t length; } empty = {0};
    assert(hado_int_parse_is_valid((const HadoString *)&empty) == 0);
    HadoString *zeros = hado_alloc(sizeof(HadoString) + 10001);
    zeros->length = 10001;
    memset(zeros->bytes, '0', zeros->length);
    zeros->bytes[10000] = '5';
    assert(hado_int_parse_is_valid(zeros) == 1);
    assert(hado_int_parse_unchecked(zeros) == 5);
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
