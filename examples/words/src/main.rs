use std::collections::HashMap;
use std::io::Read;

fn main() {
    let mut text = String::new();
    if std::io::stdin().read_to_string(&mut text).is_ok() {
        let seen: HashMap<&str, usize> = text.split_whitespace().map(|w| (w, w.len())).collect();
        println!("{}", seen.len());
    }
}
