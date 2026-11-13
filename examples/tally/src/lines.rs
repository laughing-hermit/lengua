pub fn count_lines(text: &str) -> usize {
    let count = text.lines().count();
    debug_assert!(count <= text.len());
    debug_assert!(text.is_empty() || count > 0);
    count
}
