mod arin_check;
mod build;
mod compare;
mod coverage;
mod evaluate;
mod latency;
mod sources;
mod truth;

use std::fs;
use std::net::IpAddr;
use std::path::PathBuf;
use std::time::{Duration, Instant};

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
    fn inputs(&self) -> build::Inputs<'_> {
        build::Inputs {
            data_dir: &self.data_dir,
            collectors: &self.collectors,
            vrps_url: &self.vrps_url,
            policy: self.policy(),
            geofeeds: &self.geofeeds,
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
    FetchLatency {
        #[arg(long, default_value = "data")]
        data_dir: PathBuf,
        #[arg(long, default_value_t = 4.0)]
        rate: f64,
        #[arg(long, default_value_t = 6)]
        workers: usize,
        #[arg(long)]
        max: Option<usize>,
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
        kind: compare::Kind,
        #[arg(long, default_value_t = 15)]
        top: usize,
        #[arg(long)]
        only: Option<String>,
        ours: PathBuf,
        reference: PathBuf,
    },
}

fn fetch_all(data: &DataArgs) -> Result<()> {
    let fetcher = Fetcher::new(&data.data_dir)?;
    let mut failed = Vec::new();
    for source in sources::all(&data.collectors, &data.vrps_url) {
        match fetcher.fetch(&source) {
            Ok(Outcome::Downloaded(size)) => eprintln!("{}: downloaded {size} bytes", source.name),
            Ok(Outcome::NotModified) => eprintln!("{}: not modified", source.name),
            Err(error) => {
                eprintln!(
                    "{}: failed, keeping the previous copy: {error:#}",
                    source.name
                );
                failed.push(source.name);
            }
        }
    }

    if let Err(error) = truth::fetch_violating(&fetcher) {
        eprintln!("violating probes: skipped: {error:#}");
    }

    if let Err(error) = arin_check::run(
        &data.data_dir,
        data.arin_check_sample,
        Duration::from_secs(1),
    ) {
        eprintln!("arin check: skipped: {error:#}");
    }

    build::discover(&data.data_dir, &data.geofeeds)?;
    let urls = build::geofeed_urls(&src_geofeed::list::read(&data.geofeeds)?);
    let dir = build::geofeed_dir(&data.data_dir);
    let fetcher = Fetcher::with_options(
        &dir,
        fetch::Options {
            connect_timeout: Duration::from_secs(10),
            global_timeout: Some(Duration::from_secs(120)),
            max_size: 256 << 20,
            verify_tls: false,
        },
    )?;
    let started = Instant::now();
    let stats = src_geofeed::crawl(&fetcher, &urls, data.geofeed_workers);
    eprintln!(
        "geofeeds: {} URLs, {} downloaded, {} not modified, {} failed, in {:.1?}",
        urls.len(),
        stats.downloaded,
        stats.not_modified,
        stats.failures.len(),
        started.elapsed()
    );
    let report: String = stats
        .failures
        .iter()
        .map(|(url, error)| format!("{url}\t{error}\n"))
        .collect();
    fs::write(dir.join("failures.tsv"), report)?;
    if !failed.is_empty() {
        anyhow::bail!("{} sources failed: {}", failed.len(), failed.join(", "));
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
        Command::Discover { data } => build::discover(&data.data_dir, &data.geofeeds),
        Command::Build { data, out_dir } => build::run(&data.inputs(), &out_dir),
        Command::Run { data, out_dir } => {
            fetch_all(&data)?;
            build::run(&data.inputs(), &out_dir)
        }
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
        Command::FetchLatency {
            data_dir,
            rate,
            workers,
            max,
        } => latency::run_fetch(&data_dir, rate, workers, max),
        Command::Compare {
            kind,
            top,
            only,
            ours,
            reference,
        } => compare::run(kind, &ours, &reference, top, only.as_deref()),
    }
}
