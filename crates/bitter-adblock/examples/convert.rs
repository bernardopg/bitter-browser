//! Converts an Adblock Plus list to WebKit content blocker JSON:
//! `cargo run -p bitter-adblock --example convert -- list.txt > rules.json`
fn main() {
    let path = std::env::args().nth(1).expect("usage: convert <list.txt>");
    let list = std::fs::read_to_string(&path).expect("readable list");
    let conversion = bitter_adblock::convert(&list, bitter_adblock::Scope::Full);
    eprintln!(
        "{} rules, {} filters skipped",
        conversion.rules, conversion.skipped
    );
    println!("{}", conversion.json);
}
