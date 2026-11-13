use std::fs;

fn main() {
    let mut read: u32 = 0;
    for path in std::env::args().skip(1) {
        match fs::read_to_string(&path) {
            Ok(text) => {
                print!("{text}");
                read += 1;
            }
            Err(_) => println!("cannot read {path}"),
        }
    }
    if fs::write("data/new.txt", "new\n").is_err() {
        println!("write in data: no");
    }
    if fs::write("out/count.txt", format!("{read}\n")).is_err() {
        println!("cannot write out/count.txt");
    }
}
