use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Case(std::path::PathBuf);
impl Case {
    fn new(source: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("hado-r37-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("main.hd"), source).unwrap();
        Self(dir)
    }
    fn command(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_hado")).args(args).arg(self.0.join("main.hd")).output().unwrap()
    }
    fn prints_type(&self, expected: &str) {
        let output = self.command(&["check", "--print-type"]);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        assert_eq!(String::from_utf8(output.stdout).unwrap(), format!("{expected}\n"));
    }
    fn runs(&self, expected: i64) {
        let built = self.command(&["build"]);
        assert_eq!(built.status.code(), Some(0), "{built:?}");
        assert!(built.stderr.is_empty());
        let output = Command::new(self.0.join("main")).current_dir(&self.0).output().unwrap();
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        assert_eq!(String::from_utf8(output.stdout).unwrap(), format!("{expected}\n"));
    }
}
impl Drop for Case { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }

#[test]
fn operation_result_rows_reach_the_call_without_effectful_handler_arms() {
    for (annotation, expected) in [("", "String"), (" {Error}", "String {Error}"), (" {Error, Bar}", "String {Bar, Error}")] {
        for (param, arm, call) in [("", "", "pick"), ("(n: Int)", " _", "pick 3")] {
            Case::new(&format!("effect Bar\nreply other: Int\neffect Foo\nreply pick{param}: Int{annotation}\nhandler Foo as H\nreply pick{arm} => 7\ndef main = Int.to_string (with H -> {call})\n")).prints_type(expected);
        }
    }
}

#[test]
fn operation_result_rows_remain_before_discharge() {
    Case::new("effect Foo\nreply pick: Int {Error}\ndef use (n: Int) => pick + n\ndef main = Int.to_string (use 3)\n").prints_type("String {Error, Foo}");
    Case::new("effect Foo\nreply pick: Int {Error}\ndef use (n: Int): Int {Foo, Error} => pick + n\ndef main = Int.to_string (use 3)\n").prints_type("String {Error, Foo}");
}

#[test]
fn value_parameters_shadow_operations_in_mono() {
    Case::new("def f (raise: Int): Int => raise\ndef main = Int.to_string (f 3)\n").runs(3);
}

#[test]
fn function_parameters_shadow_operations_in_infer_and_mono() {
    Case::new("def inc (n: Int): Int => n + 1\ndef f (raise: Fn(Int => Int)): Int => raise 3\ndef main = Int.to_string (f inc)\n").runs(4);
}

#[test]
fn let_bindings_shadow_operations() {
    Case::new("def main = Int.to_string (let raise = 5 -> raise + 1)\n").runs(6);
}

#[test]
fn user_operation_parameters_shadow_operations() {
    Case::new("effect Foo\nreply tick: Int\ndef f (tick: Int): Int => tick\ndef main = Int.to_string (f 3)\n").runs(3);
}

#[test]
fn shadowed_capture_is_a_local_let_value() {
    Case::new("effect Foo\ncapture tick(value: Int): Unit\ndef f (tick: Int): Int => let x = tick -> x + 1\ndef main = Int.to_string (f 8)\n").runs(9);
}

#[test]
fn prelude_reply_defaults_cover_every_operation() {
    let checked = hado::check_source("def main = 0\n", "main.hd").unwrap();
    let replies: std::collections::HashSet<_> = checked.operations.iter()
        .filter(|(_, kind)| **kind == hado::parser::OpKind::Reply)
        .map(|(name, _)| name).collect();
    assert!(!replies.is_empty());
    assert_eq!(replies, checked.default_operations.keys().collect());
    for definition in checked.default_operations.values() {
        assert!(checked.definitions.contains_key(definition), "{definition}");
    }
}

#[test]
fn default_definitions_are_required_and_checked_after_inference() {
    let prefix = "effect Error\nabort raise(error: Exception)\neffect World\n";
    for (operation, definition, expected) in [
        ("reply pick: Int", "", Some("E204")),
        ("reply pick: Int", "def default_pick = \"wrong\"", Some("E305")),
        ("reply pick: Int", "def default_pick = raise { message = \"x\" }", Some("E305")),
        ("reply pick: Int {Error}", "def default_pick = 11", None),
        ("reply pick(n: Int): Int {Error}", "def default_pick n => n + 1", None),
        ("reply pick(n: Int): Int", "def default_pick = 11", Some("E301")),
        ("reply pick: Int", "def default_pick n => n + 1", Some("E301")),
    ] {
        let source = format!("{prefix}{operation}\n{definition}\ndef main = 0\n");
        let result = hado::check_source(&source, "/tmp/stdlib/default_probe.hd");
        match expected {
            Some(code) => assert_eq!(result.err().expect(&source).code, Some(code.to_owned()), "{source}"),
            None => assert!(result.is_ok(), "{source}: {:?}", result.err()),
        }
    }
}

#[test]
fn runtime_preserves_argv_and_copies_length_delimited_environment_names() {
    let case = Case::new("def main = 0\n");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = case.0.join("probe.c");
    let object = case.0.join("probe.o");
    let executable = case.0.join("probe");
    std::fs::write(&source, r#"
#include "hado_runtime.h"
#include <assert.h>
#include <string.h>
int32_t hado_entry(void) {
    assert(hado_argv_count(0) == 3);
    const HadoString *first = hado_argv_get(0);
    assert(first->length == 5 && memcmp(first->bytes, "first", 5) == 0);
    assert(hado_argv_get(1)->length == 0);
    assert(hado_argv_get(2)->length == 1 && hado_argv_get(2)->bytes[0] == 255);
    struct { size_t length; unsigned char bytes[9]; } key = {7, "R37_KEYX"};
    assert(hado_env_has((const HadoString *)&key) == 1);
    const HadoString *value = hado_env_get((const HadoString *)&key);
    assert(value->length == 6 && memcmp(value->bytes, "copied", 6) == 0);
    key.length = 8;
    assert(hado_env_has((const HadoString *)&key) == 1);
    value = hado_env_get((const HadoString *)&key);
    assert(value->length == 4 && memcmp(value->bytes, "tail", 4) == 0);
    key.bytes[0] = 'Z';
    assert(hado_env_has((const HadoString *)&key) == 0);
    return 0;
}
"#).unwrap();
    let compiled = Command::new("cc").args(["-std=c11", "-Wall", "-Werror", "-c"])
        .arg(&source).arg("-I").arg(root.join("runtime")).arg("-o").arg(&object).output().unwrap();
    assert!(compiled.status.success(), "{}", String::from_utf8_lossy(&compiled.stderr));
    hado::link_executable(&object, &executable).unwrap();
    use std::os::unix::ffi::OsStrExt;
    let output = Command::new(executable).args(["first", ""])
        .arg(std::ffi::OsStr::from_bytes(&[255])).env("R37_KEY", "copied").env("R37_KEYX", "tail")
        .current_dir(&case.0).output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty() && output.stderr.is_empty(), "{output:?}");
}

#[test]
fn io_is_a_prelude_effect_with_all_four_defaults() {
    Case::new("def main = [List.length args, Bytes.length read_stdin]\n").prints_type("Int List {Env, Error, Io}");
    let checked = hado::check_source("def main = 0\n", "main.hd").unwrap();
    for operation in ["read_file", "write_file", "read_dir", "read_stdin"] {
        assert!(checked.default_operations.contains_key(operation), "{operation}");
    }
}

#[test]
fn io_runtime_classifies_errno_and_copies_sorts_and_filters_directory_names() {
    let case = Case::new("def main = 0\n");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = case.0.join("io_probe.c");
    let executable = case.0.join("io_probe");
    std::fs::create_dir(case.0.join("entries")).unwrap();
    for name in ["z", "m", "a", ".hidden", "日本語"] {
        std::fs::write(case.0.join("entries").join(name), b"data").unwrap();
    }
    std::fs::write(case.0.join("file"), b"data").unwrap();
    std::fs::write(&source, r#"
#include "hado_runtime.c"
#include <assert.h>
int32_t hado_entry(void) {
    assert(io_status_from_errno(ENOENT) == 1);
    assert(io_status_from_errno(EACCES) == 2);
    assert(io_status_from_errno(ENOTDIR) == 3);
    assert(io_status_from_errno(EISDIR) == 4);
    assert(io_status_from_errno(EIO) == 4);
    struct { size_t length; unsigned char bytes[10]; } path = {7, "entriesXX"};
    assert(hado_io_read_dir((const HadoString *)&path) == 0);
    assert(hado_io_dir_count(0) == 5);
    const char *expected[] = {".hidden", "a", "m", "z", "日本語"};
    for (int i = 0; i < 5; i++) {
        HadoString *name = hado_io_dir_get(i);
        assert(name->length == strlen(expected[i]));
        assert(memcmp(name->bytes, expected[i], name->length) == 0);
    }
    path.length = 4;
    memcpy(path.bytes, "fileXX", 6);
    assert(hado_io_read_file((const HadoString *)&path) == 0);
    HadoString *bytes = hado_io_take_bytes(0);
    assert(bytes->length == 4 && memcmp(bytes->bytes, "data", 4) == 0);
    assert(hado_io_write_file((const HadoString *)&path, bytes_from_c_string("replaced")) == 0);
    assert(hado_io_read_file((const HadoString *)&path) == 0);
    HadoString *written = hado_io_take_bytes(0);
    assert(written->length == 8 && memcmp(written->bytes, "replaced", 8) == 0);
    assert(bytes->length == 4 && memcmp(bytes->bytes, "data", 4) == 0);
    assert(hado_io_read_dir((const HadoString *)&path) == 3);
    return 0;
}
"#).unwrap();
    let compiled = Command::new("cc").args(["-std=c11", "-Wall", "-Werror"])
        .arg(&source).arg("-I").arg(root.join("runtime")).arg("-lgc").arg("-o").arg(&executable).output().unwrap();
    assert!(compiled.status.success(), "{}", String::from_utf8_lossy(&compiled.stderr));
    let output = Command::new(executable).current_dir(&case.0).output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty() && output.stderr.is_empty(), "{output:?}");
}
