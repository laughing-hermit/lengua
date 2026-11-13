use std::hash::{BuildHasher, RandomState};
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    println!("{seconds}");
    println!("{}", RandomState::new().hash_one(0_u8));
}
