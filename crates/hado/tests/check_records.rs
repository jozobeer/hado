use hado::{check_source, types::Ty};

#[test]
fn nominal_record_construction_and_projection() {
    let source = "type User = { name: String; age: Int }\ndef main = let user = { age = 3; name = \"a\" } -> user.name";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::String);
    let source = "type User = { name: String; age: Int }\ndef main = { age = true; name = \"a\" }";
    assert_eq!(
        check_source(source, "t").err().unwrap().code.as_deref(),
        Some("E305")
    );
}

#[test]
fn record_construction_requires_unique_field_set() {
    let source = "type A = { x: Int }\ntype B = { x: Bool }\ndef main = { x = 1 }";
    assert_eq!(
        check_source(source, "t").err().unwrap().code.as_deref(),
        Some("E304")
    );
    assert!(check_source("def main = { x = 1 }", "t").is_err());
}

#[test]
fn typed_record_patterns_bind_field_types() {
    let source = "type User = { name: String; age: Int }\ndef name (user: User) => if user | { name = n } => n\ndef main = name { name = \"a\"; age = 3 }";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::String);
    let source = "type User = { name: String }\ndef main = let { name } = { name = \"a\" } -> name";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::String);
}

#[test]
fn unique_fields_infer_record_parameters() {
    let source = "type User = { name: String; age: Int }\ndef name user => user.name\ndef main = name { name = \"a\"; age = 3 }";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::String);
    let source = "type T Box = { value: T }\ndef unbox box => box.value\ndef a = unbox { value = 1 }\ndef main = (a > 0) && unbox { value = true }";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Bool);
}

#[test]
fn unresolved_projection_waits_for_call_site() {
    let source = "type A = { value: Int } type B = { value: Bool; extra: Int } def get x => x.value def main = get { value = true; extra = 1 } && { value = 1 }.value == 1";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Bool);
}
