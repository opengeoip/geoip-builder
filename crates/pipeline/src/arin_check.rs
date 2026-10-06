use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use fetch::{Fetcher, Source};
use src_arin::NetRange;

use model::date::today;
use model::hash::fnv1a;

use crate::{Log, sources};

pub fn run(data_dir: &Path, sample: usize, interval: Duration, log: &mut Log<'_>) -> Result<()> {
    let source = sources::arin_geofeed_inetnums();
    let file = File::open(source.path(data_dir))?;
    let (ranges, _, _) = src_arin::parse(BufReader::new(file))?;
    let day = today();
    let mut candidates: Vec<&NetRange> = ranges.iter().filter(|r| r.geofeed().is_some()).collect();
    candidates.sort_by_key(|r| fnv1a(day, r.handle.as_bytes()));
    candidates.truncate(sample);

    let fetcher = Fetcher::new(data_dir.join("arin-rdap"))?;
    let (mut matching, mut differing, mut failed) = (0, 0, 0);
    for (index, range) in candidates.iter().enumerate() {
        if index > 0 {
            thread::sleep(interval);
        }
        let check = Source::new(format!("{}.json", range.handle), src_arin::rdap_url(range));
        if let Err(error) = fetcher.fetch(&check) {
            failed += 1;
            log(format!("arin check: {}: {error:#}", range.handle));
            if fetch::is_rate_limited(&error) {
                break;
            }
            continue;
        }
        let live = src_arin::rdap_geofeed(BufReader::new(File::open(check.path(fetcher.dir()))?))?;
        if live.as_deref() == range.geofeed() {
            matching += 1;
        } else {
            differing += 1;
            log(format!(
                "arin check: {} lists {:?}, RDAP now says {:?}",
                range.handle,
                range.geofeed(),
                live
            ));
        }
    }
    log(format!(
        "arin check: {matching} matching, {differing} differing, {failed} failed, out of {} sampled",
        candidates.len()
    ));
    Ok(())
}
