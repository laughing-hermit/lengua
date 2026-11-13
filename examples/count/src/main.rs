use std::io::Read;

fn main() {
    let mut text = String::new();
    if std::io::stdin().read_to_string(&mut text).is_err() {
        std::process::exit(1);
    }
    println!("{}", text.split_whitespace().count());
}
