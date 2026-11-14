const LONGEST: usize = 40;

#[must_use]
pub fn title_error(title: &str) -> String {
    let count = length(title);
    debug_assert!(count <= title.chars().count());
    let error = if count == 0 {
        "A note has a title."
    } else if count > LONGEST {
        "A title has 40 characters or fewer."
    } else {
        ""
    };
    debug_assert!(error.is_empty() || count == 0 || count > LONGEST);
    error.to_string()
}

fn length(title: &str) -> usize {
    let count = title.trim().chars().count();
    debug_assert!(count <= title.len());
    debug_assert!(title.trim().len() <= title.len());
    count
}
