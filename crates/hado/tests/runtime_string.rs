#[test]
fn utf8_strings_count_scalars_and_preserve_embedded_nul() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir().join(format!("hado-string-runtime-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("test.c");
    let object = dir.join("test.o");
    let executable = dir.join("test");
    std::fs::write(&source, r#"
#include "hado_runtime.h"
#include <assert.h>
#include <string.h>
int32_t hado_entry(void) {
    struct { size_t length; unsigned char bytes[7]; } a = {7, {'A', 0, 0xe3, 0x81, 0x82, 'B', 'C'}};
    struct { size_t length; unsigned char bytes[4]; } b = {4, {0xf0, 0x9f, 0x98, 0x80}};
    struct { size_t length; } empty = {0};
    const HadoString *left = (const HadoString *)&a;
    const HadoString *right = (const HadoString *)&b;
    assert(hado_string_length(left) == 5);
    assert(hado_string_length(right) == 1);
    HadoString *joined = hado_string_concat(left, right);
    assert(joined->length == 11);
    assert(hado_string_length(joined) == 6);
    assert(memcmp(joined->bytes, a.bytes, 7) == 0);
    assert(memcmp(joined->bytes + 7, b.bytes, 4) == 0);
    assert(a.length == 7 && b.length == 4);
    assert(hado_string_compare(left, joined) == -1);
    assert(hado_string_compare(right, left) == 1);
    assert(hado_string_compare(left, left) == 0);
    assert(hado_string_compare(hado_string_concat((const HadoString *)&empty, left), left) == 0);
    return 0;
}
"#).unwrap();
    let status = std::process::Command::new("cc").args(["-std=c11", "-Wall", "-Werror", "-c"]).arg(&source).arg("-I").arg(root.join("runtime")).arg("-o").arg(&object).status().unwrap();
    assert!(status.success());
    hado::link_executable(&object, &executable).unwrap();
    assert!(std::process::Command::new(executable).status().unwrap().success());
    std::fs::remove_dir_all(dir).unwrap();
}
