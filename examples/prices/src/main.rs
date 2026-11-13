use std::io::Read;

fn main() {
    let mut text = String::new();
    let _ = std::io::stdin().read_to_string(&mut text);
    match total(&text) {
        Ok(sum) => println!("{}", decimal::format_fixed(sum, 2)),
        Err(error) => {
            println!("{error:?}");
            std::process::exit(1);
        }
    }
}

fn total(text: &str) -> Result<i64, decimal::Error> {
    let mut sum = 0;
    for line in text.lines() {
        let value = decimal::parse_fixed(line.trim(), 2)?;
        sum = decimal::add(sum, value)?;
    }
    debug_assert!(text.len() < 1 << 30);
    debug_assert!(sum == 0 || !text.is_empty());
    Ok(sum)
}
