use crate::sha256;

pub const CASES_DRAWN: usize = 20;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum End {
    #[default]
    Exited,
    OutOfTime,
    OutOfMemory,
    Trapped,
}

impl End {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Exited => "exited",
            Self::OutOfTime => "out of time",
            Self::OutOfMemory => "out of memory",
            Self::Trapped => "trapped",
        }
    }

    pub(crate) fn parse(text: &str) -> Option<Self> {
        [
            Self::Exited,
            Self::OutOfTime,
            Self::OutOfMemory,
            Self::Trapped,
        ]
        .into_iter()
        .find(|end| end.name() == text)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Range {
    Integers {
        name: String,
        low: i64,
        high: i64,
    },
    Text {
        name: String,
        low: u64,
        high: u64,
        characters: Vec<char>,
    },
}

impl Range {
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Integers { name, .. } | Self::Text { name, .. } => name,
        }
    }

    fn bounds(&self) -> (i128, i128) {
        match self {
            Self::Integers { low, high, .. } => (i128::from(*low), i128::from(*high)),
            Self::Text { low, high, .. } => (low.to_owned().into(), high.to_owned().into()),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Test {
    pub file: String,
    pub line: usize,
    pub behaviour: String,
    pub name: String,
    pub screen: bool,
    pub ranges: Vec<Range>,
    pub stdin: Option<String>,
    pub arguments: Option<Vec<String>>,
    pub files: Vec<(String, String)>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub exit_code: Option<u8>,
    pub end: End,
    pub expect_files: Vec<(String, String)>,
    pub call: Option<String>,
    pub value: Option<String>,
    pub servers: Vec<(String, String)>,
    pub clients: Vec<(u16, String)>,
    pub answer: Option<String>,
    pub request: Option<String>,
    pub request_headers: Vec<(String, String)>,
    pub request_body: Option<String>,
    pub status: Option<u16>,
    pub expect_headers: Vec<(String, String)>,
    pub expect_body: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    pub values: Vec<(String, String)>,
    pub test: Test,
}

struct Draw {
    state: u64,
}

impl Draw {
    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, span: u128) -> u128 {
        u128::from(self.next()) % span.max(1)
    }
}

fn boundaries(low: i128, high: i128) -> Vec<i128> {
    let mut out: Vec<i128> = Vec::new();
    for value in [low, low + 1, high - 1, high] {
        if value >= low && value <= high && !out.contains(&value) {
            out.push(value);
        }
    }
    out
}

fn pick(low: i128, high: i128, boundary: Option<usize>, draw: &mut Draw) -> i128 {
    if let Some(k) = boundary {
        let list = boundaries(low, high);
        if let Some(value) = list.get(k.min(list.len().saturating_sub(1))) {
            return *value;
        }
    }
    let span = u128::try_from(high - low + 1).unwrap_or(1);
    low + i128::try_from(draw.below(span)).unwrap_or(0)
}

fn drawn(range: &Range, boundary: Option<usize>, draw: &mut Draw) -> String {
    let (low, high) = range.bounds();
    let value = pick(low, high, boundary, draw);
    match range {
        Range::Integers { .. } => value.to_string(),
        Range::Text { characters, .. } => {
            let count = u128::try_from(characters.len()).unwrap_or(1);
            (0..value)
                .map(|_| {
                    let index = usize::try_from(draw.below(count)).unwrap_or(0);
                    characters.get(index).copied().unwrap_or(' ')
                })
                .collect()
        }
    }
}

fn seed(test: &Test) -> u64 {
    let named = format!("{}\0{}\0{:?}", test.behaviour, test.name, test.ranges);
    let hex = sha256::hex(named.as_bytes());
    u64::from_str_radix(hex.get(..16).unwrap_or("0"), 16).unwrap_or(0)
}

pub(crate) fn fill(text: &str, values: &[(String, String)]) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push('}');
            }
            '{' => {
                let name: String = chars.by_ref().take_while(|n| *n != '}').collect();
                if let Some((_, value)) = values.iter().find(|(n, _)| *n == name) {
                    out.push_str(value);
                }
            }
            _ => out.push(c),
        }
    }
    out
}

fn filled(test: &Test, values: &[(String, String)]) -> Test {
    let one = |written: &String| fill(written, values);
    let pairs = |list: &[(String, String)]| -> Vec<(String, String)> {
        list.iter()
            .map(|(path, written)| (path.clone(), fill(written, values)))
            .collect()
    };
    Test {
        stdin: test.stdin.as_ref().map(one),
        arguments: test
            .arguments
            .as_ref()
            .map(|list| list.iter().map(one).collect()),
        files: pairs(&test.files),
        stdout: test.stdout.as_ref().map(one),
        stderr: test.stderr.as_ref().map(one),
        expect_files: pairs(&test.expect_files),
        call: test.call.as_ref().map(one),
        value: test.value.as_ref().map(one),
        servers: pairs(&test.servers),
        clients: test
            .clients
            .iter()
            .map(|(port, sent)| (*port, fill(sent, values)))
            .collect(),
        answer: test.answer.as_ref().map(one),
        request: test.request.as_ref().map(one),
        request_headers: pairs(&test.request_headers),
        request_body: test.request_body.as_ref().map(one),
        expect_headers: pairs(&test.expect_headers),
        expect_body: test.expect_body.as_ref().map(one),
        ..test.clone()
    }
}

#[must_use]
pub fn cases(test: &Test) -> Vec<Case> {
    if test.ranges.is_empty() {
        return vec![Case {
            values: Vec::new(),
            test: test.clone(),
        }];
    }
    let mut draw = Draw { state: seed(test) };
    let edges = test
        .ranges
        .iter()
        .map(|r| {
            let (low, high) = r.bounds();
            boundaries(low, high).len()
        })
        .max()
        .unwrap_or(0);
    let mut sets: Vec<Vec<(String, String)>> = Vec::new();
    for k in 0..edges + CASES_DRAWN {
        let boundary = (k < edges).then_some(k);
        let values: Vec<(String, String)> = test
            .ranges
            .iter()
            .map(|r| (r.name().to_string(), drawn(r, boundary, &mut draw)))
            .collect();
        if !sets.contains(&values) {
            sets.push(values);
        }
    }
    sets.into_iter()
        .map(|values| Case {
            test: filled(test, &values),
            values,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{CASES_DRAWN, End, Range, Test, cases, fill};

    fn numbers(low: i64, high: i64) -> Test {
        Test {
            behaviour: "Say the size".to_string(),
            name: "small numbers".to_string(),
            ranges: vec![Range::Integers {
                name: "n".to_string(),
                low,
                high,
            }],
            arguments: Some(vec!["{n}".to_string()]),
            stdout: Some("{n} is {{small}}\n".to_string()),
            ..Test::default()
        }
    }

    #[test]
    fn boundary_values_come_first_then_values_from_a_fixed_seed() {
        let found = cases(&numbers(0, 9));
        let first: Vec<&str> = found
            .iter()
            .take(4)
            .map(|c| c.values[0].1.as_str())
            .collect();
        assert_eq!(first, ["0", "1", "8", "9"]);
        assert!(found.len() <= 4 + CASES_DRAWN);
        assert!(found.len() > 4);
        for case in &found {
            let n: i64 = case.values[0].1.parse().expect("a number");
            assert!((0..=9).contains(&n));
            assert_eq!(case.test.arguments, Some(vec![n.to_string()]));
            assert_eq!(case.test.stdout, Some(format!("{n} is {{small}}\n")));
        }
        assert_eq!(found, cases(&numbers(0, 9)));
    }

    #[test]
    fn a_range_of_one_value_gives_one_case() {
        assert_eq!(cases(&numbers(5, 5)).len(), 1);
        let wide = cases(&numbers(i64::MIN, i64::MAX));
        assert_eq!(wide[0].values[0].1, i64::MIN.to_string());
        assert_eq!(wide[3].values[0].1, i64::MAX.to_string());
    }

    #[test]
    fn text_is_drawn_from_its_characters_with_its_lengths_first() {
        let test = Test {
            ranges: vec![Range::Text {
                name: "w".to_string(),
                low: 1,
                high: 8,
                characters: vec!['a', 'b', 'c'],
            }],
            stdin: Some("{w}\n".to_string()),
            ..Test::default()
        };
        let found = cases(&test);
        let lengths: Vec<usize> = found
            .iter()
            .take(4)
            .map(|c| c.values[0].1.chars().count())
            .collect();
        assert_eq!(lengths, [1, 2, 7, 8]);
        for case in &found {
            assert!(case.values[0].1.chars().all(|c| "abc".contains(c)));
            assert_eq!(case.test.stdin, Some(format!("{}\n", case.values[0].1)));
        }
    }

    #[test]
    fn an_example_is_one_case_with_its_braces_as_written() {
        let test = Test {
            stdout: Some("{x}\n".to_string()),
            ..Test::default()
        };
        let found = cases(&test);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].test.stdout, Some("{x}\n".to_string()));
        assert_eq!(
            fill("{a}-{{b}}", &[("a".to_string(), "1".to_string())]),
            "1-{b}"
        );
        assert_eq!(End::parse("out of time"), Some(End::OutOfTime));
        assert_eq!(End::parse("stopped"), None);
    }
}
