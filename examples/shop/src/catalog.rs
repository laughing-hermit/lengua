#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item {
    pub name: &'static str,
    pub price: i64,
}

const ITEMS: [Item; 3] = [
    Item {
        name: "tea",
        price: 250,
    },
    Item {
        name: "cake",
        price: 320,
    },
    Item {
        name: "jam",
        price: 475,
    },
];

pub fn find(name: &str) -> Option<Item> {
    let found = ITEMS.iter().find(|item| item.name == name).copied();
    debug_assert!(found.is_none_or(|item| item.price > 0));
    debug_assert_eq!(ITEMS.len(), 3);
    found
}

pub fn listing() -> json::Value {
    let items: Vec<json::Value> = ITEMS
        .iter()
        .map(|item| {
            json::Value::Object(vec![
                ("name".to_string(), json::Value::Text(item.name.to_string())),
                (
                    "price".to_string(),
                    json::Value::Text(decimal::format_fixed(item.price, 2)),
                ),
            ])
        })
        .collect();
    debug_assert_eq!(items.len(), ITEMS.len());
    debug_assert_ne!(items.len(), 0);
    json::Value::List(items)
}
