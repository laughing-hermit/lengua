use crate::catalog::{self, Item};

const MOST: i64 = 1000;

pub fn parse_query(query: &str) -> Result<Vec<(Item, i64)>, String> {
    let mut lines = Vec::new();
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (name, count) = pair.split_once('=').unwrap_or((pair, ""));
        let item = catalog::find(name).ok_or_else(|| format!("no item {name}"))?;
        let quantity = decimal::parse_integer(count)
            .ok()
            .filter(|quantity| *quantity > 0 && *quantity <= MOST)
            .ok_or_else(|| format!("no count for {name}"))?;
        lines.push((item, quantity));
    }
    debug_assert!(lines.len() <= query.len());
    debug_assert!(lines.iter().all(|(_, quantity)| *quantity > 0));
    Ok(lines)
}

pub fn net(lines: &[(Item, i64)]) -> Result<i64, decimal::Error> {
    let mut sum = 0;
    for (item, quantity) in lines {
        sum = decimal::add(sum, decimal::multiply(item.price, *quantity)?)?;
    }
    debug_assert!(sum >= 0);
    debug_assert!(lines.is_empty() || sum > 0);
    Ok(sum)
}
