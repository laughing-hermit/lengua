fn main() {
    for argument in std::env::args().skip(1) {
        println!("{argument}");
    }
}
