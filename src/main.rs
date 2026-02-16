use clap::Parser;

#[derive(Parser)]
struct Cli {
    name: String,
}

fn main() {
    let args = Cli::parse();
    println!("name: {:?}", args.name);
}
