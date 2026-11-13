use std::fs;
use std::path::Path;

fn rust_files(folder: &Path, out: &mut Vec<(String, String)>) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let text = fs::read_to_string(&path).expect("a Rust file");
            out.push((path.to_string_lossy().replace('\\', "/"), text));
        }
    }
}

#[test]
fn lengua_seal_has_the_one_unsafe_call_of_lengua() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for entry in fs::read_dir(&crates).expect("the crates").flatten() {
        rust_files(&entry.path().join("src"), &mut files);
    }
    assert!(files.len() > 10, "{}", files.len());
    let word = ["unsafe", " {"].concat();
    let found: Vec<(String, usize)> = files
        .iter()
        .map(|(path, text)| (path.clone(), text.matches(word.as_str()).count()))
        .filter(|(_, count)| *count > 0)
        .collect();
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].0.ends_with("lengua-seal/src/lib.rs"), "{found:?}");
    assert_eq!(found[0].1, 1, "{found:?}");
    let manifests: Vec<String> = fs::read_dir(&crates)
        .expect("the crates")
        .flatten()
        .filter_map(|e| fs::read_to_string(e.path().join("Cargo.toml")).ok())
        .collect();
    let allowed = manifests
        .iter()
        .filter(|m| !m.contains("[lints]\nworkspace = true"))
        .count();
    assert_eq!(allowed, 1);
}
