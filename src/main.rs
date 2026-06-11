use clap::{Parser, Subcommand};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "truv_compression")]
#[command(about = "A custom compression utility (.truv)", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Compress {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    Decompress {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Compress { input, output } => {
            let mut input_file = File::open(input)?;
            let mut raw_data = Vec::new();
            input_file.read_to_end(&mut raw_data)?;
            let output_file = File::create(output)?;
            let writer = BufWriter::new(output_file);
            println!("Compressing {}...", input.display());
            let bytes_written = truv_compression::compress::compress(&raw_data, writer)?;
            println!("Compression finished. Written {} bytes.", bytes_written);
        }
        Commands::Decompress { input, output } => {
            let input_file = File::open(input)?;
            let reader = BufReader::new(input_file);
            let output_file = File::create(output)?;
            let writer = BufWriter::new(output_file);
            println!("Decompressing {}...", input.display());
            truv_compression::decompress::decompress(reader, writer)?;
            println!(
                "Decompression finished successfully. Saved to {}",
                output.display()
            );
        }
    }

    Ok(())
}
