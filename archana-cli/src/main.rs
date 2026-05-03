use anyhow::Result;
use archana_core::Archive;
use clap::Parser;
use std::process;

mod args;
use args::{Cli, Command};

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = Cli::parse();
    
    match args.command {
        Command::List { archive, verbose } => {
            let mut ar = Archive::open(&archive, Default::default())?;
            for entry in ar.entries()? {
                let entry = entry?;
                if verbose {
                    println!(
                        "{} {:>10} {:>10} {:?}",
                        entry.name,
                        entry.size_uncompressed,
                        entry.size_compressed,
                        entry.compression
                    );
                } else {
                    println!("{}", entry.name);
                }
            }
        }
        
        Command::Extract { archive, dest, .. } => {
            let dest = dest.unwrap_or_else(|| ".".to_string());
            let mut ar = Archive::open(&archive, Default::default())?;
            let stats = ar.extract_all(&dest, Default::default())?;
            println!(
                "Extracted {} files ({} bytes -> {} bytes)",
                stats.files, stats.bytes_out, stats.bytes_in
            );
        }
        
        Command::Add { archive, files, format, compression } => {
            let format = format
                .as_ref()
                .and_then(|f| archana_core::Format::from_extension(f))
                .unwrap_or(archana_core::Format::Zip);
            
            let mut ar = Archive::create(&archive, format, Default::default())?;
            
            for file in &files {
                ar.append_file(file, Default::default())?;
            }
            
            let stats = ar.finish()?;
            println!("Added {} files", stats.files);
        }
        
        Command::Test { archive } => {
            let mut ar = Archive::open(&archive, Default::default())?;
            let count = ar.entries()?.count();
            println!("Archive OK ({} entries)", count);
        }
        
        Command::Info { archive } => {
            let mut ar = Archive::open(&archive, Default::default())?;
            let entries: Vec<_> = ar.entries()?.filter_map(|e| e.ok()).collect();
            let total_size: u64 = entries.iter().map(|e| e.size_uncompressed).sum();
            println!("Archive: {}", archive);
            println!("Entries: {}", entries.len());
            println!("Total size: {} bytes", total_size);
        }
    }
    
    Ok(())
}