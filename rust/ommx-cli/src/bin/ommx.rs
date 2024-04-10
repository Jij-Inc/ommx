use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Inspect { image_name_or_path: String },
}

fn main() {
    let cli = Cli::parse();

    match cli.verbose {
        0 => println!("Verbose level is WARN"),
        1 => println!("Verbose level is INFO"),
        2 => println!("Verbose level is DEBUG"),
        _ => panic!("Too many verbose flags. Don't be crazy."),
    }

    match &cli.command {
        Some(Commands::Inspect { image_name_or_path }) => {
            dbg!(image_name_or_path);
        }
        None => {}
    }
}
