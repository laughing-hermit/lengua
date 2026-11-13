fn main() {
    let mut blocks: Vec<Vec<u8>> = Vec::new();
    let mut total: u64 = 0;
    loop {
        let block = vec![1_u8; 1 << 20];
        total += u64::from(block.first().copied().unwrap_or(0));
        blocks.push(block);
        if total == u64::MAX {
            break;
        }
    }
    println!("{} {total}", blocks.len());
}
