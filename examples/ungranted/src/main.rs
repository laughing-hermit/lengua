use std::hash::{BuildHasher, RandomState};

fn main() {
    let number = RandomState::new().hash_one(0_u8);
    if std::fs::write("note.txt", format!("{number}\n")).is_err() {
        println!("cannot write note.txt");
    }
    println!("{number}");
}
