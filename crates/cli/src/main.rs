mod build;
mod compare;
mod sources;

use std::net::IpAddr;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use fetch::{Fetcher, Outcome};

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Args)]
struct DataArgs {
    #[arg(long, default_value = "data")]
    data_dir: PathBuf,
    #[arg(long = "collector", default_value = "rrc00")]
    collectors: Vec<String>,
    #[arg(long, default_value = sources::DEFAULT_VRPS_URL)]
    vrps_url: String,
}

#[derive(Subcommand)]
enum Command {
    Fetch {
        #[command(flatten)]
        data: DataArgs,
    },
    Build {
        #[command(flatten)]
        data: DataArgs,
        #[arg(long, default_value = "out")]
        out_dir: PathBuf,
    },
    Run {
        #[command(flatten)]
        data: DataArgs,
        #[arg(long, default_value = "out")]
        out_dir: PathBuf,
    },
    Lookup {
        database: PathBuf,
        addresses: Vec<IpAddr>,
    },
    Compare {
        #[arg(long, value_enum)]
        kind: compare::Kind,
        #[arg(long, default_value_t = 15)]
        top: usize,
        ours: PathBuf,
        reference: PathBuf,
    },
}

fn fetch_all(data: &DataArgs) -> Result<()> {
    let fetcher = Fetcher::new(&data.data_dir)?;
    for source in sources::all(&data.collectors, &data.vrps_url) {
        match fetcher.fetch(&source)? {
            Outcome::Downloaded(size) => eprintln!("{}: downloaded {size} bytes", source.name),
            Outcome::NotModified => eprintln!("{}: not modified", source.name),
        }
    }
    Ok(())
}

fn lookup(database: PathBuf, addresses: Vec<IpAddr>) -> Result<()> {
    let reader = maxminddb::Reader::open_readfile(database)?;
    for address in addresses {
        let result = reader.lookup(address)?;
        let record: Option<serde_json::Value> = result.decode()?;
        let network = result.network().map(|n| n.to_string()).unwrap_or_default();
        println!(
            "{address} {network} {}",
            record.map(|r| r.to_string()).unwrap_or_else(|| "-".into())
        );
    }
    Ok(())
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Fetch { data } => fetch_all(&data),
        Command::Build { data, out_dir } => {
            build::run(&data.data_dir, &out_dir, &data.collectors, &data.vrps_url)
        }
        Command::Run { data, out_dir } => {
            fetch_all(&data)?;
            build::run(&data.data_dir, &out_dir, &data.collectors, &data.vrps_url)
        }
        Command::Lookup {
            database,
            addresses,
        } => lookup(database, addresses),
        Command::Compare {
            kind,
            top,
            ours,
            reference,
        } => compare::run(kind, &ours, &reference, top),
    }
}
