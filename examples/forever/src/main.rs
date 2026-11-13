fn main() {
    let mut count: u64 = 0;
    loop {
        count = std::hint::black_box(count.wrapping_add(1));
    }
}
