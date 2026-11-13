fn main() {
    let argument = std::env::args().nth(1).unwrap_or_default();
    let Ok(number) = argument.parse::<u32>() else {
        std::process::exit(2);
    };
    println!("{}", size(number));
}

fn size(number: u32) -> &'static str {
    let name = if number <= 9 { "small" } else { "large" };
    assert!(!name.is_empty());
    assert_eq!(name.len(), 5);
    name
}
