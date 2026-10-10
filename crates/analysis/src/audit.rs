use std::net::{IpAddr, Ipv4Addr};

use anyhow::{Result, bail};
use ipnetwork::IpNetwork;
use maxminddb::{Reader, WithinOptions, path};

use crate::lookup;

pub const Z95: f64 = 1.96;

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    pub fn below(&mut self, bound: u64) -> u64 {
        ((u128::from(self.next_u64()) * u128::from(bound)) >> 64) as u64
    }
}

pub fn ipv4_ranges(reader: &Reader<Vec<u8>>, hosting: bool) -> Result<Vec<(u32, u32)>> {
    let scope: IpNetwork = "0.0.0.0/0".parse()?;
    let mut ranges = Vec::new();
    for item in reader.within(scope, WithinOptions::default())? {
        let item = item?;
        if hosting
            && !item
                .decode_path::<bool>(&path!["is_hosting_provider"])?
                .unwrap_or(false)
        {
            continue;
        }
        let IpNetwork::V4(network) = item.network()? else {
            continue;
        };
        let start = u32::from(network.network());
        let end = start + ((1u64 << (32 - network.prefix())) - 1) as u32;
        ranges.push((start, end));
    }
    ranges.sort_unstable();
    Ok(ranges)
}

pub fn size(ranges: &[(u32, u32)]) -> u64 {
    ranges
        .iter()
        .map(|(start, end)| u64::from(end - start) + 1)
        .sum()
}

pub struct Space<'a> {
    ranges: &'a [(u32, u32)],
    ends: Vec<u64>,
}

impl<'a> Space<'a> {
    pub fn new(ranges: &'a [(u32, u32)]) -> Self {
        let ends = ranges
            .iter()
            .scan(0u64, |total, (start, end)| {
                *total += u64::from(end - start) + 1;
                Some(*total)
            })
            .collect();
        Space { ranges, ends }
    }

    pub fn pick(&self, rng: &mut Rng) -> Option<Ipv4Addr> {
        let offset = rng.below(*self.ends.last()?);
        let index = self.ends.partition_point(|&end| end <= offset);
        let before = if index == 0 { 0 } else { self.ends[index - 1] };
        Some(Ipv4Addr::from(
            self.ranges[index].0 + (offset - before) as u32,
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draw {
    pub address: Ipv4Addr,
    pub flagged: bool,
}

pub fn sample(
    routed: &[(u32, u32)],
    hosting: &Reader<Vec<u8>>,
    per_stratum: usize,
    rng: &mut Rng,
) -> Result<Vec<Draw>> {
    let space = Space::new(routed);
    let mut draws = Vec::new();
    let (mut flagged, mut unflagged) = (0, 0);
    let mut attempts = 0;
    while flagged < per_stratum || unflagged < per_stratum {
        attempts += 1;
        if attempts > per_stratum * 10_000 {
            bail!("could not draw {per_stratum} addresses on each side");
        }
        let Some(address) = space.pick(rng) else {
            bail!("no routed address to draw from");
        };
        let is_flagged = lookup::hosting_provider(hosting, IpAddr::V4(address))?;
        let count = if is_flagged {
            &mut flagged
        } else {
            &mut unflagged
        };
        if *count < per_stratum {
            *count += 1;
            draws.push(Draw {
                address,
                flagged: is_flagged,
            });
        }
    }
    Ok(draws)
}

pub fn wilson(successes: usize, total: usize) -> (f64, f64, f64) {
    if total == 0 {
        return (0.0, 0.0, 0.0);
    }
    let n = total as f64;
    let p = successes as f64 / n;
    let z2 = Z95 * Z95;
    let center = (p + z2 / (2.0 * n)) / (1.0 + z2 / n);
    let margin = Z95 * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt() / (1.0 + z2 / n);
    (p, center - margin, center + margin)
}

#[derive(Debug, Default, PartialEq)]
pub struct Estimate {
    pub flagged_hosting: usize,
    pub flagged_judged: usize,
    pub unflagged_access: usize,
    pub unflagged_judged: usize,
    pub unknown: usize,
    pub hosting_addresses_flagged: f64,
    pub hosting_addresses_missed: f64,
}

impl Estimate {
    pub fn precision(&self) -> (f64, f64, f64) {
        wilson(self.flagged_hosting, self.flagged_judged)
    }

    pub fn negative_precision(&self) -> (f64, f64, f64) {
        wilson(self.unflagged_access, self.unflagged_judged)
    }

    pub fn recall(&self) -> f64 {
        let total = self.hosting_addresses_flagged + self.hosting_addresses_missed;
        if total == 0.0 {
            0.0
        } else {
            self.hosting_addresses_flagged / total
        }
    }
}

pub fn estimate(
    verdicts: &[(bool, Option<bool>)],
    flagged_space: u64,
    unflagged_space: u64,
) -> Estimate {
    let mut estimate = Estimate::default();
    for &(flagged, hosting) in verdicts {
        match (flagged, hosting) {
            (_, None) => estimate.unknown += 1,
            (true, Some(hosting)) => {
                estimate.flagged_judged += 1;
                estimate.flagged_hosting += usize::from(hosting);
            }
            (false, Some(hosting)) => {
                estimate.unflagged_judged += 1;
                estimate.unflagged_access += usize::from(!hosting);
            }
        }
    }
    let (precision, _, _) = estimate.precision();
    let (negative, _, _) = estimate.negative_precision();
    estimate.hosting_addresses_flagged = precision * flagged_space as f64;
    estimate.hosting_addresses_missed = (1.0 - negative) * unflagged_space as f64;
    estimate
}

#[cfg(test)]
mod tests {
    use mmdb_writer::Value;

    use super::*;
    use crate::testing::database;

    #[test]
    fn draws_addresses_in_proportion_to_range_size() {
        let ranges = [(0, 99), (1000, 1899)];
        let space = Space::new(&ranges);
        let mut rng = Rng::new(7);
        let in_large = (0..10_000)
            .filter(|_| u32::from(space.pick(&mut rng).unwrap()) >= 1000)
            .count();
        assert!(
            (0..1000)
                .map(|_| u32::from(space.pick(&mut rng).unwrap()))
                .all(|a| a <= 99 || (1000..=1899).contains(&a))
        );
        assert!((8_800..9_200).contains(&in_large), "{in_large}");
        assert_eq!(size(&ranges), 1000);
    }

    #[test]
    fn samples_each_side_and_reads_ranges() {
        let reader = database(&[
            (
                "192.0.2.0/25",
                Value::map([("is_hosting_provider", Value::Bool(true))]),
            ),
            (
                "192.0.2.128/25",
                Value::map([("is_hosting_provider", Value::Bool(false))]),
            ),
        ]);
        let hosting = ipv4_ranges(&reader, true).unwrap();
        assert_eq!(
            hosting,
            [(
                u32::from(Ipv4Addr::new(192, 0, 2, 0)),
                u32::from(Ipv4Addr::new(192, 0, 2, 127))
            )]
        );
        let routed = ipv4_ranges(&reader, false).unwrap();
        assert_eq!(size(&routed), 256);
        let draws = sample(&routed, &reader, 5, &mut Rng::new(1)).unwrap();
        assert_eq!(draws.iter().filter(|d| d.flagged).count(), 5);
        assert_eq!(draws.iter().filter(|d| !d.flagged).count(), 5);
        assert!(
            draws
                .iter()
                .all(|d| d.flagged == (d.address.octets()[3] < 128))
        );
    }

    #[test]
    fn estimates_precision_both_ways_and_recall() {
        let verdicts = [
            (true, Some(true)),
            (true, Some(true)),
            (true, Some(true)),
            (true, Some(false)),
            (false, Some(false)),
            (false, Some(false)),
            (false, Some(false)),
            (false, Some(true)),
            (false, None),
        ];
        let estimate = estimate(&verdicts, 1000, 3000);
        assert_eq!(estimate.unknown, 1);
        assert_eq!(estimate.precision().0, 0.75);
        assert_eq!(estimate.negative_precision().0, 0.75);
        assert_eq!(estimate.hosting_addresses_flagged, 750.0);
        assert_eq!(estimate.hosting_addresses_missed, 750.0);
        assert_eq!(estimate.recall(), 0.5);
        let (p, low, high) = wilson(8, 10);
        assert_eq!(p, 0.8);
        assert!((low - 0.490).abs() < 0.001 && (high - 0.943).abs() < 0.001);
    }
}
