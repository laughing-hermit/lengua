fn main() {
    let mut blocks: Vec<Vec<u8>> = Vec::new();
    for _ in 0..u32::MAX {
        blocks.push(std::hint::black_box(vec![1_u8; 1 << 20]));
    }
    println!("{}", blocks.len());
}
