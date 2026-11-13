use std::time::Duration;

fn main() {
    std::thread::sleep(Duration::from_secs(30));
    println!("awake");
}
