use clap::Parser;

#[derive(Parser)]
struct Cli {
    name: Option<String>,
}

fn main() {
    let args = Cli::parse();

    if let Some(name) = args.name {
        println!("name: {:?}", name);
    } else {
        println!("interactive mode");
    }
}
