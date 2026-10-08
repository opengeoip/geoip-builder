mod candidates;
mod compare;
mod coverage;
mod evaluate;

use std::net::IpAddr;
use std::path::{Path, PathBuf};

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
    #[arg(long, default_value = sources::CATALOG_URL)]
    geofeeds: String,
}

impl DataArgs {
    fn catalog(&self) -> PathBuf {
        pipeline::catalog::path(&self.data_dir, &self.geofeeds)
    }

    fn inputs<'a>(&'a self, catalog: &'a Path) -> Inputs<'a> {
        Inputs {
            data_dir: &self.data_dir,
            collectors: &self.collectors,
            vrps_url: &self.vrps_url,
            policy: self.policy(),
            geofeeds: catalog,
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
        #[arg(long, default_value = "data")]
        data_dir: PathBuf,
        #[arg(long, default_value = "geofeeds.csv")]
        output: PathBuf,
        #[arg(long)]
        manual: Option<PathBuf>,
        #[arg(long)]
        fetch: bool,
    },
    CheckCatalog {
        #[arg(required = true)]
        catalogs: Vec<PathBuf>,
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
        add_to: Option<PathBuf>,
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
        #[arg(long)]
        json: Option<PathBuf>,
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

fn check_catalogs(catalogs: &[PathBuf]) -> Result<()> {
    for catalog in catalogs {
        let report = pipeline::catalog::check(catalog)?;
        println!(
            "{}: {} rows, {} geofeeds, {} anchored rows",
            catalog.display(),
            report.rows,
            report.urls,
            report.anchored
        );
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
        Command::Fetch { data } => pipeline::fetch::fetch(&data.fetch(), &mut log),
        Command::Discover {
            data_dir,
            output,
            manual,
            fetch,
        } => pipeline::catalog::discover(
            &pipeline::catalog::DiscoverOptions {
                data_dir: &data_dir,
                output: &output,
                manual: manual.as_deref(),
                fetch,
            },
            &mut log,
        ),
        Command::CheckCatalog { catalogs } => check_catalogs(&catalogs),
        Command::Build { data, out_dir } => {
            let catalog = data.catalog();
            pipeline::build::build(&data.inputs(&catalog), &out_dir, &mut log)
        }
        Command::Run { data, out_dir } => {
            pipeline::fetch::fetch(&data.fetch(), &mut log)?;
            let catalog = data.catalog();
            pipeline::build::build(&data.inputs(&catalog), &out_dir, &mut log)
        }
        Command::Candidates {
            data,
            truth,
            country,
            asn,
            coverage,
            output,
            probe,
            add_to,
        } => candidates::run(
            &pipeline::candidates::Options {
                data_dir: &data.data_dir,
                truth: &truth,
                country: &country,
                asn: &asn,
                coverage: &coverage,
                geofeeds: &data.catalog(),
                probe,
            },
            &output,
            add_to.as_deref(),
        ),
        Command::Coverage { data, output, top } => {
            let catalog = data.catalog();
            coverage::run(&data.inputs(&catalog), &output, top)
        }
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
            json,
            databases,
        } => evaluate::run(
            &evaluate::Options {
                data_dir: &data_dir,
                truth: &truth,
                only: only.as_deref(),
                keep_suspicious,
                top,
                json: json.as_deref(),
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
