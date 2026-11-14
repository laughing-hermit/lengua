use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::notes::{error, field, quoted};

const FILE: &str = "data/users.txt";
const ROUNDS: u32 = 20_000;
const HOUR: u64 = 3600;
const LONGEST_NAME: usize = 20;
const SHORTEST_PASSWORD: usize = 8;

type User = (String, String);

pub fn register(body: &str) -> (u16, String) {
    let name = field(body, "name").unwrap_or_default();
    let password = field(body, "password").unwrap_or_default();
    debug_assert!(body.len() < 1 << 20);
    if !good_name(&name) {
        return (400, error("A name has 1 to 20 small letters."));
    }
    if password.chars().count() < SHORTEST_PASSWORD {
        return (400, error("A password has 8 characters or more."));
    }
    let mut users = load();
    if users.iter().any(|(known, _)| *known == name) {
        return (409, error("This name has an account."));
    }
    let salt = logins::to_hex(&wasip2::random::random::get_random_bytes(8));
    users.push((name.clone(), logins::hash_password(&password, &salt, ROUNDS)));
    debug_assert!(!users.is_empty());
    if !save(&users) {
        return (500, error("The account was not saved."));
    }
    (201, format!("{{\"name\":{}}}", quoted(&name)))
}

pub fn login(body: &str) -> (u16, String) {
    let name = field(body, "name").unwrap_or_default();
    let password = field(body, "password").unwrap_or_default();
    let found = load().into_iter().find(|(known, _)| *known == name);
    debug_assert!(found.as_ref().is_none_or(|(known, _)| *known == name));
    debug_assert!(body.len() < 1 << 20);
    match found {
        Some((_, hash)) if logins::verify_password(&password, &hash) => {
            let token = logins::sign_token(&name, now().saturating_add(HOUR), &key());
            (200, format!("{{\"token\":{}}}", quoted(&token)))
        }
        _ => (401, error("The name or the password is wrong.")),
    }
}

pub fn me(authorization: &str) -> (u16, String) {
    let token = authorization.strip_prefix("Bearer ").unwrap_or_default();
    let checked = logins::check_token(token, now(), &key());
    debug_assert!(checked.as_ref().is_err() || checked.as_ref().is_ok_and(|n| !n.is_empty()));
    debug_assert!(token.len() <= authorization.len());
    match checked {
        Ok(name) => (200, format!("{{\"name\":{}}}", quoted(&name))),
        Err(_) => (401, error("Sign in first.")),
    }
}

fn good_name(name: &str) -> bool {
    let size = name.chars().count();
    let good = (1..=LONGEST_NAME).contains(&size) && name.chars().all(|c| c.is_ascii_lowercase());
    debug_assert!(!good || !name.is_empty());
    debug_assert!(!good || !name.contains('\t'));
    good
}

fn now() -> u64 {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    debug_assert!(seconds < u64::MAX);
    debug_assert!(HOUR > 0);
    seconds
}

fn key() -> String {
    let key = std::env::var("SESSION_KEY").unwrap_or_default();
    debug_assert!(key.len() < 1 << 16);
    debug_assert!(!key.contains('\0'));
    key
}

fn load() -> Vec<User> {
    let text = fs::read_to_string(FILE).unwrap_or_default();
    let users: Vec<User> = text
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .map(|(name, hash)| (name.to_string(), hash.to_string()))
        .collect();
    debug_assert!(users.len() <= text.lines().count());
    debug_assert!(users.iter().all(|(name, _)| !name.contains('\t')));
    users
}

fn save(users: &[User]) -> bool {
    let text: String = users
        .iter()
        .map(|(name, hash)| format!("{name}\t{hash}\n"))
        .collect();
    debug_assert_eq!(text.lines().count(), users.len());
    debug_assert!(!users.is_empty());
    fs::write(FILE, text).is_ok()
}
