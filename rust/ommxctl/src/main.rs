use clap::{Parser, Subcommand};
use ocipkg::ImageName;
use std::path::Path;

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
    Inspect {
        /// Container image name or the path of OCI archive
        image_name_or_path: String,
    },
}

fn inspect(image_name_or_path: &str) {
    let path: &Path = image_name_or_path.as_ref();
    if path.exists() {
        log::debug!("Regarded as a filesystem path: {}", path.display());
        if !path.is_file() {
            panic!("Not a file: {}", path.display());
        }

        dbg!(path);

        return;
    }
    if path.is_file() {
        dbg!("It is a file");
        return;
    }
    if let Ok(image) = ImageName::parse(image_name_or_path) {
        dbg!(image);
        return;
    };
    panic!("Not an image name or valid path: {}", image_name_or_path);
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
            inspect(image_name_or_path);
        }
        None => {}
    }
}
