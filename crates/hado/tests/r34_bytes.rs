use hado::check_source;
use hado::mono::Type;
use hado::types::Ty;

#[test]
fn bytes_type_round_trips_between_tys_and_mono() {
    assert_eq!(Type::try_from(&Ty::Bytes), Ok(Type::Bytes));
    assert_eq!(Ty::from(&Type::Bytes), Ty::Bytes);
}

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
    assert_eq!(RuntimeFunction::BytesLength.declaration().symbol, "hado_bytes_length");
    assert_eq!(RuntimeFunction::BytesGet.declaration().symbol, "hado_bytes_get");
    assert_eq!(RuntimeFunction::BytesIsUtf8.declaration().symbol, "hado_bytes_is_utf8");
    assert_eq!(RuntimeFunction::BytesDisplay.declaration().symbol, "hado_bytes_display");
    assert_eq!(RuntimeFunction::BytesOfString.declaration().symbol, "hado_bytes_of_string");
    assert_eq!(
        RuntimeFunction::BytesToStringUnchecked.declaration().symbol,
        "hado_bytes_to_string_unchecked"
    );
    assert_eq!(RuntimeFunction::BytesAlloc.declaration().symbol, "hado_bytes_alloc");
    assert_eq!(
        RuntimeFunction::BytesFillUnchecked.declaration().symbol,
        "hado_bytes_fill_unchecked"
    );
    assert_eq!(RuntimeFunction::BytesIsUtf8.declaration().result, Some(AbiType::I8));
}

#[test]
fn bytes_is_utf8_and_display_at_the_c_level() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir().join(format!("hado-bytes-runtime-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("test.c");
    let object = dir.join("test.o");
    let executable = dir.join("test");
    std::fs::write(&source, r#"
#include "hado_runtime.h"
#include <assert.h>
#include <string.h>
typedef struct { size_t length; unsigned char bytes[5]; } Utf8Probe;

static Utf8Probe utf8_bytes(uint32_t scalar, size_t width) {
    Utf8Probe encoded = {width, {0}};
    if (width == 1) {
        encoded.bytes[0] = scalar;
        return encoded;
    }
    encoded.bytes[0] = (0xff << (8 - width)) | (scalar >> (6 * (width - 1)));
    for (size_t index = 1; index < width; ++index) {
        encoded.bytes[index] = 0x80 | ((scalar >> (6 * (width - 1 - index))) & 0x3f);
    }
    return encoded;
}

static void assert_utf8_round_trip(const Utf8Probe *probe) {
    const HadoString *original = (const HadoString *)probe;
    assert(hado_bytes_is_utf8(original) == 1);
    HadoString *result = hado_bytes_of_string(hado_bytes_to_string_unchecked(original));
    assert(result->length == probe->length);
    assert(memcmp(result->bytes, probe->bytes, probe->length) == 0);
}

int32_t hado_entry(void) {
    struct { size_t length; unsigned char bytes[3]; } valid = {3, {0xe3, 0x81, 0x82}};
    struct { size_t length; unsigned char bytes[1]; } invalid = {1, {0xff}};
    struct { size_t length; unsigned char bytes[2]; } overlong = {2, {0xc0, 0xaf}};
    struct { size_t length; unsigned char bytes[1]; } lone = {1, {0x80}};
    struct { size_t length; unsigned char bytes[3]; } surrogate = {3, {0xed, 0xa0, 0x80}};
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
    assert(hado_bytes_is_utf8((const HadoString *)&invalid) == 0);
    assert(hado_bytes_is_utf8((const HadoString *)&overlong) == 0);
    assert(hado_bytes_is_utf8((const HadoString *)&lone) == 0);
    assert(hado_bytes_is_utf8((const HadoString *)&surrogate) == 0);
    for (size_t width = 3; width <= 5; ++width) {
        Utf8Probe noncanonical = utf8_bytes(0, width);
        assert(hado_bytes_is_utf8((const HadoString *)&noncanonical) == 0);
    }
    const uint32_t shortest[] = {0x41, 0x80, 0x800, 0x10000};
    for (size_t index = 0; index < sizeof(shortest) / sizeof(shortest[0]); ++index) {
        Utf8Probe encoded = utf8_bytes(shortest[index], index + 1);
        assert_utf8_round_trip(&encoded);
        for (size_t position = 1; position < encoded.length; ++position) {
            Utf8Probe invalid_continuation = encoded;
            invalid_continuation.bytes[position] = 0x41;
            assert(hado_bytes_is_utf8((const HadoString *)&invalid_continuation) == 0);
        }
    }
    for (uint32_t scalar = 0x10ffff; scalar <= 0x110000; ++scalar) {
        Utf8Probe boundary = utf8_bytes(scalar, 4);
        assert(hado_bytes_is_utf8((const HadoString *)&boundary) == (scalar <= 0x10ffff));
        if (scalar <= 0x10ffff) assert_utf8_round_trip(&boundary);
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
"#).unwrap();
    let status = std::process::Command::new("cc").args(["-std=c11", "-Wall", "-Werror", "-c"]).arg(&source).arg("-I").arg(root.join("runtime")).arg("-o").arg(&object).status().unwrap();
    assert!(status.success());
    hado::link_executable(&object, &executable).unwrap();
    assert!(std::process::Command::new(executable).status().unwrap().success());
    std::fs::remove_dir_all(dir).unwrap();
}
