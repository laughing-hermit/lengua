fn main() {
    let mut count: u64 = 0;
    for step in 0..u64::MAX {
        count = std::hint::black_box(count.wrapping_add(step));
    }
    println!("{count}");
}
