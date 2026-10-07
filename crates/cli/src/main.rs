mod candidates;
mod compare;
mod coverage;
mod evaluate;

use std::net::IpAddr;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use pipeline::build::Inputs;
use pipeline::fetch::FetchOptions;
use pipeline::sources;

pub fn log(line: String) {
    eprintln!("{line}");
}

#[derive(Parser)]
#[command(name = "geoip-builder", version, about)]
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
    #[arg(long)]
    rpki_valid_only: bool,
    #[arg(long, default_value_t = 32)]
    geofeed_workers: usize,
    #[arg(long, default_value_t = 10)]
    arin_check_sample: usize,
    #[arg(long, default_value = "catalog/geofeeds.csv")]
    geofeeds: PathBuf,
}

impl DataArgs {
    fn inputs(&self) -> Inputs<'_> {
        Inputs {
            data_dir: &self.data_dir,
            collectors: &self.collectors,
            vrps_url: &self.vrps_url,
            policy: self.policy(),
            geofeeds: &self.geofeeds,
        }
    }

    fn fetch(&self) -> FetchOptions<'_> {
        FetchOptions {
            data_dir: &self.data_dir,
            collectors: &self.collectors,
            vrps_url: &self.vrps_url,
            geofeeds: &self.geofeeds,
            geofeed_workers: self.geofeed_workers,
            arin_check_sample: self.arin_check_sample,
        }
    }

    fn policy(&self) -> merge::RpkiPolicy {
        if self.rpki_valid_only {
            merge::RpkiPolicy::ValidOnly
        } else {
            merge::RpkiPolicy::RejectInvalid
        }
    }
}

#[derive(Subcommand)]
enum Command {
    Fetch {
        #[command(flatten)]
        data: DataArgs,
    },
    Discover {
        #[command(flatten)]
        data: DataArgs,
    },
    Candidates {
        #[command(flatten)]
        data: DataArgs,
        #[arg(long, default_value = "data/atlas-probes.json.bz2")]
        truth: PathBuf,
        #[arg(long, default_value = "out/country.mmdb")]
        country: PathBuf,
        #[arg(long, default_value = "out/asn.mmdb")]
        asn: PathBuf,
        #[arg(long, default_value = "out/geofeed-coverage.csv")]
        coverage: PathBuf,
        #[arg(long, default_value = "out/geofeed-candidates.csv")]
        output: PathBuf,
        #[arg(long, default_value_t = 50)]
        probe: usize,
        #[arg(long)]
        add: bool,
    },
    Coverage {
        #[command(flatten)]
        data: DataArgs,
        #[arg(long, default_value = "out/geofeed-coverage.csv")]
        output: PathBuf,
        #[arg(long, default_value_t = 20)]
        top: usize,
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
    Evaluate {
        #[arg(long, default_value = "data/atlas-probes.json.bz2")]
        truth: PathBuf,
        #[arg(long, default_value = "data")]
        data_dir: PathBuf,
        #[arg(long)]
        only: Option<String>,
        #[arg(long)]
        keep_suspicious: bool,
        #[arg(long, default_value_t = 10)]
        top: usize,
        #[arg(required = true)]
        databases: Vec<PathBuf>,
    },
    Compare {
        #[arg(long, value_enum)]
        kind: compare::KindArg,
        #[arg(long, default_value_t = 15)]
        top: usize,
        #[arg(long)]
        only: Option<String>,
        ours: PathBuf,
        reference: PathBuf,
    },
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
        Command::Fetch { data } => pipeline::fetch::fetch(&data.fetch(), &mut log),
        Command::Discover { data } => {
            pipeline::catalog::discover(&data.data_dir, &data.geofeeds, &mut log)
        }
        Command::Build { data, out_dir } => {
            pipeline::build::build(&data.inputs(), &out_dir, &mut log)
        }
        Command::Run { data, out_dir } => {
            pipeline::fetch::fetch(&data.fetch(), &mut log)?;
            pipeline::build::build(&data.inputs(), &out_dir, &mut log)
        }
        Command::Candidates {
            data,
            truth,
            country,
            asn,
            coverage,
            output,
            probe,
            add,
        } => candidates::run(
            &pipeline::candidates::Options {
                data_dir: &data.data_dir,
                truth: &truth,
                country: &country,
                asn: &asn,
                coverage: &coverage,
                geofeeds: &data.geofeeds,
                probe,
            },
            &output,
            add,
        ),
        Command::Coverage { data, output, top } => coverage::run(&data.inputs(), &output, top),
        Command::Lookup {
            database,
            addresses,
        } => lookup(database, addresses),
        Command::Evaluate {
            truth,
            data_dir,
            only,
            keep_suspicious,
            top,
            databases,
        } => evaluate::run(
            &evaluate::Options {
                data_dir: &data_dir,
                truth: &truth,
                only: only.as_deref(),
                keep_suspicious,
                top,
            },
            &databases,
        ),
        Command::Compare {
            kind,
            top,
            only,
            ours,
            reference,
        } => compare::run(kind, &ours, &reference, top, only.as_deref()),
    }
}
