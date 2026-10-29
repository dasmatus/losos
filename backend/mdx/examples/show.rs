// Prints a document's Rendered JSON: `cargo run --example show -- file.paper`.
fn main() {
    let path = std::env::args().nth(1).expect("a file");
    let source = std::fs::read_to_string(&path).expect("readable");
    let flavour = losos_mdx::Flavour::for_name(&path).unwrap_or(losos_mdx::Flavour::Mdx);
    match losos_mdx::render(&source, flavour, &losos_mdx::Plain) {
        Ok(r) => println!("{}", serde_json::to_string_pretty(&r).unwrap()),
        Err(e) => eprintln!("{e}"),
    }
}
