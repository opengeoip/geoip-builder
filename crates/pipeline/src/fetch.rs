use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Result;
use fetch::{Fetcher, Outcome};

use crate::catalog::{discover, geofeed_dir, geofeed_urls};
use crate::{Log, arin_check, sources, truth};

pub struct FetchOptions<'a> {
    pub data_dir: &'a Path,
    pub collectors: &'a [String],
    pub vrps_url: &'a str,
    pub geofeeds: &'a Path,
    pub geofeed_workers: usize,
    pub arin_check_sample: usize,
}

fn bulk_sources(options: &FetchOptions<'_>, fetcher: &Fetcher, log: &mut Log<'_>) -> Vec<String> {
    let mut failed = Vec::new();
    for source in sources::all(options.collectors, options.vrps_url) {
        match fetcher.fetch(&source) {
            Ok(Outcome::Downloaded(size)) => {
                log(format!("{}: downloaded {size} bytes", source.name))
            }
            Ok(Outcome::NotModified) => log(format!("{}: not modified", source.name)),
            Err(error) => {
                log(format!(
                    "{}: failed, keeping the previous copy: {error:#}",
                    source.name
                ));
                failed.push(source.name);
            }
        }
    }
    failed
}

pub fn geofeed_fetcher(data_dir: &Path, global_timeout: Duration) -> Result<Fetcher> {
    Fetcher::with_options(
        geofeed_dir(data_dir),
        fetch::Options {
            connect_timeout: Duration::from_secs(10),
            global_timeout: Some(global_timeout),
            max_size: 256 << 20,
            verify_tls: false,
        },
    )
}

fn geofeeds(options: &FetchOptions<'_>, log: &mut Log<'_>) -> Result<()> {
    let urls = geofeed_urls(&src_geofeed::list::read(options.geofeeds)?);
    let fetcher = geofeed_fetcher(options.data_dir, Duration::from_secs(120))?;
    let started = Instant::now();
    let stats = src_geofeed::crawl(&fetcher, &urls, options.geofeed_workers);
    log(format!(
        "geofeeds: {} URLs, {} downloaded, {} not modified, {} failed, in {:.1?}",
        urls.len(),
        stats.downloaded,
        stats.not_modified,
        stats.failures.len(),
        started.elapsed()
    ));
    let report: String = stats
        .failures
        .iter()
        .map(|(url, error)| format!("{url}\t{error}\n"))
        .collect();
    fs::write(fetcher.dir().join("failures.tsv"), report)?;
    Ok(())
}

pub fn fetch(options: &FetchOptions<'_>, log: &mut Log<'_>) -> Result<()> {
    let fetcher = Fetcher::new(options.data_dir)?;
    let failed = bulk_sources(options, &fetcher, log);
    if let Err(error) = truth::fetch_violating(&fetcher, log) {
        log(format!("violating probes: skipped: {error:#}"));
    }
    if let Err(error) = arin_check::run(
        options.data_dir,
        options.arin_check_sample,
        Duration::from_secs(1),
        log,
    ) {
        log(format!("arin check: skipped: {error:#}"));
    }
    discover(options.data_dir, options.geofeeds, log)?;
    geofeeds(options, log)?;
    if !failed.is_empty() {
        anyhow::bail!("{} sources failed: {}", failed.len(), failed.join(", "));
    }
    Ok(())
}
