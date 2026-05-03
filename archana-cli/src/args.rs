use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "archana")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Add {
        archive: String,
        files: Vec<String>,
        #[arg(short, long)]
        format: Option<String>,
        #[arg(short, long, default_value = "6")]
        compression: Option<i32>,
    },
    Extract {
        archive: String,
        #[arg(short, long)]
        dest: Option<String>,
        #[arg(short, long)]
        password: Option<String>,
    },
    List {
        archive: String,
        #[arg(short, long)]
        verbose: bool,
    },
    Test {
        archive: String,
    },
    Info {
        archive: String,
    },
}