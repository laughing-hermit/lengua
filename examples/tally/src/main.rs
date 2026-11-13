mod lines;
mod report;
mod words;

use std::io::Read;

fn main() {
    let mut text = String::new();
    if std::io::stdin().read_to_string(&mut text).is_ok() {
        let line = report::format_report(
            lines::count_lines(&text),
            words::count_words(&text),
            words::maximum_length(&text),
        );
        println!("{line}");
    }
}
