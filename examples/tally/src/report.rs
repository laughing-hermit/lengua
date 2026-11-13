pub fn format_report(lines: usize, words: usize, maximum: usize) -> String {
    let line = format!("lines {lines}, words {words}, maximum {maximum}");
    debug_assert!(line.starts_with("lines "));
    debug_assert!(line.len() >= 27);
    line
}
