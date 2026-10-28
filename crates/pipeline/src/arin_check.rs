use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use fetch::{Fetcher, Source};
use ipnet::IpNet;
use src_arin::NetRange;

use model::date::today;
use model::hash::fnv1a;
use model::range_to_networks;

use crate::{Log, sources};

pub struct Check {
    pub sampled: usize,
    pub matching: usize,
    pub differing: usize,
    pub failed: usize,
    pub withdrawn: usize,
    pub withdrawn_networks: Vec<IpNet>,
}

pub fn run(data_dir: &Path, sample: usize, interval: Duration, log: &mut Log<'_>) -> Result<Check> {
    let source = sources::arin_geofeed_inetnums();
    let file = File::open(source.path(data_dir))?;
    let (ranges, _, _) = src_arin::parse(BufReader::new(file))?;
    let day = today();
    let mut candidates: Vec<&NetRange> = ranges.iter().filter(|r| r.geofeed().is_some()).collect();
    candidates.sort_by_key(|r| fnv1a(day, r.handle.as_bytes()));
    candidates.truncate(sample);

    let fetcher = Fetcher::new(data_dir.join("arin-rdap"))?;
    let (mut matching, mut differing, mut failed) = (0, 0, 0);
    let mut withdrawn = 0;
    let mut withdrawn_networks = Vec::new();
    for (index, range) in candidates.iter().enumerate() {
        if index > 0 {
            thread::sleep(interval);
        }
        let live = match lookup(&fetcher, range, interval) {
            Ok(live) => live,
            Err(error) => {
                failed += 1;
                log(format!("arin check: {}: {error:#}", range.handle));
                if fetch::is_rate_limited(&error) {
                    break;
                }
                continue;
            }
        };
        match live {
            Some(live) if live.as_deref() == range.geofeed() => matching += 1,
            Some(live) => {
                differing += 1;
                log(format!(
                    "arin check: {} lists {:?}, RDAP now says {:?}",
                    range.handle,
                    range.geofeed(),
                    live
                ));
            }
            None => {
                withdrawn += 1;
                withdrawn_networks
                    .extend(range_to_networks(range.start, range.end).unwrap_or_default());
                log(format!(
                    "arin check: {} lists {:?}, RDAP no longer has this network",
                    range.handle,
                    range.geofeed()
                ));
            }
        }
    }
    log(format!(
        "arin check: {matching} matching, {differing} differing, {withdrawn} withdrawn, {failed} failed, out of {} sampled",
        candidates.len()
    ));
    Ok(Check {
        sampled: candidates.len(),
        matching,
        differing,
        failed,
        withdrawn,
        withdrawn_networks,
    })
}

const MAX_HOPS: usize = 8;

fn lookup(
    fetcher: &Fetcher,
    range: &NetRange,
    interval: Duration,
) -> Result<Option<Option<String>>> {
    let mut url = src_arin::rdap_url(range);
    for hop in 0..MAX_HOPS {
        if hop > 0 {
            thread::sleep(interval);
        }
        let source = Source::new(format!("{}-{hop}.json", range.handle), url);
        fetcher.fetch(&source)?;
        let network =
            src_arin::rdap_network(BufReader::new(File::open(source.path(fetcher.dir()))?))?;
        if network.handle == range.handle {
            return Ok(Some(network.geofeed));
        }
        if network.start < range.start || network.end > range.end {
            return Ok(None);
        }
        match network.up {
            Some(up) => url = up,
            None => return Ok(None),
        }
    }
    Ok(None)
}
