use json::Value;

#[must_use]
pub fn parse_vector(text: &str) -> Option<Vec<f64>> {
    let Ok(Value::List(items)) = json::parse(text.trim()) else {
        return None;
    };
    let mut out = Vec::with_capacity(items.len());
    for item in &items {
        let Value::Number(number) = item else {
            return None;
        };
        out.push(number.parse::<f64>().ok().filter(|n| n.is_finite())?);
    }
    debug_assert_eq!(out.len(), items.len());
    debug_assert!(out.iter().all(|n| n.is_finite()));
    Some(out)
}

#[must_use]
pub fn dot(a: &[f64], b: &[f64]) -> f64 {
    let total = a.iter().zip(b).map(|(x, y)| x * y).sum();
    debug_assert!(a.len() < usize::MAX);
    debug_assert!(b.len() < usize::MAX);
    total
}

#[must_use]
pub fn cosine(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let size = (dot(a, a) * dot(b, b)).sqrt();
    debug_assert!(size >= 0.0 || size.is_nan());
    debug_assert_eq!(a.len(), b.len());
    if size > 0.0 { dot(a, b) / size } else { 0.0 }
}

#[must_use]
pub fn nearest(query: &[f64], items: &[Vec<f64>], count: usize) -> Vec<usize> {
    let mut scored: Vec<(usize, f64)> = items
        .iter()
        .enumerate()
        .map(|(index, item)| (index, cosine(query, item)))
        .collect();
    scored.sort_by(|x, y| y.1.total_cmp(&x.1).then(x.0.cmp(&y.0)));
    let found: Vec<usize> = scored.iter().take(count).map(|(index, _)| *index).collect();
    debug_assert!(found.len() <= count);
    debug_assert!(found.len() <= items.len());
    found
}
