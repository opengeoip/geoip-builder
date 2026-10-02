use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use fetch::{Fetcher, Source};
use src_arin::NetRange;

use crate::sources;

fn score(handle: &str, day: u64) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325 ^ day;
    for byte in handle.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub fn run(data_dir: &Path, sample: usize, interval: Duration) -> Result<()> {
    let source = sources::arin_geofeed_inetnums();
    let file = File::open(source.path(data_dir))?;
    let (ranges, _, _) = src_arin::parse(BufReader::new(file))?;
    let day = sources::today();
    let mut candidates: Vec<&NetRange> = ranges.iter().filter(|r| r.geofeed().is_some()).collect();
    candidates.sort_by_key(|r| score(&r.handle, day));
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
            eprintln!("arin check: {}: {error:#}", range.handle);
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
            eprintln!(
                "arin check: {} lists {:?}, RDAP now says {:?}",
                range.handle,
                range.geofeed(),
                live
            );
        }
    }
    eprintln!(
        "arin check: {matching} matching, {differing} differing, {failed} failed, out of {} sampled",
        candidates.len()
    );
    Ok(())
}
