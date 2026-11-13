pub fn count_words(text: &str) -> usize {
    let count = text.split_whitespace().count();
    debug_assert!(count <= text.len());
    debug_assert!(count == 0 || !text.trim().is_empty());
    count
}

pub fn maximum_length(text: &str) -> usize {
    let most = text
        .split_whitespace()
        .map(|word| word.chars().count())
        .max()
        .unwrap_or(0);
    debug_assert!(most <= text.len());
    debug_assert!(most == 0 || count_words(text) > 0);
    most
}
