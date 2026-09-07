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
