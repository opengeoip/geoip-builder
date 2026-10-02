use std::collections::BTreeMap;
use std::io::BufRead;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use anyhow::Result;
use fetch::{Fetcher, Outcome, Source};
use ipnet::IpNet;
use model::Location;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ParseStats {
    pub entries: usize,
    pub invalid: usize,
}

pub fn parse<R: BufRead>(mut reader: R) -> Result<(Vec<Location>, ParseStats)> {
    let mut locations = Vec::new();
    let mut stats = ParseStats::default();
    let mut buffer = Vec::new();
    while reader.read_until(b'\n', &mut buffer)? > 0 {
        let line = String::from_utf8_lossy(&buffer);
        let line = line.split('#').next().unwrap_or_default().trim();
        if !line.is_empty() {
            match parse_line(line) {
                Some(location) => {
                    stats.entries += 1;
                    locations.push(location);
                }
                None => stats.invalid += 1,
            }
        }
        buffer.clear();
    }
    Ok((locations, stats))
}

fn field(fields: &[&str], index: usize) -> Option<String> {
    let value = fields.get(index)?.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn parse_line(line: &str) -> Option<Location> {
    let fields: Vec<&str> = line.split(',').collect();
    let network: IpNet = fields[0].trim().parse().ok()?;
    let country = match field(&fields, 1) {
        Some(code) => {
            let code = code.to_ascii_uppercase();
            if code.len() != 2 || !code.bytes().all(|b| b.is_ascii_alphabetic()) {
                return None;
            }
            Some(code)
        }
        None => None,
    };
    let region = field(&fields, 2).and_then(|region| {
        let region = region.to_ascii_uppercase();
        match (&country, region.split_once('-')) {
            (Some(country), Some((prefix, code))) if prefix == country && !code.is_empty() => {
                Some(code.to_string())
            }
            _ => None,
        }
    });
    Some(Location {
        network: network.trunc(),
        country,
        region,
        city: field(&fields, 3),
        postal: field(&fields, 4),
    })
}

pub fn cache_name(url: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in url.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}.csv")
}

pub fn source(url: &str) -> Source {
    Source::new(cache_name(url), url)
}

fn host(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.split(['/', ':', '?']).next().unwrap_or(rest)
}

#[derive(Debug, Default)]
pub struct CrawlStats {
    pub downloaded: usize,
    pub not_modified: usize,
    pub failures: Vec<(String, String)>,
}

pub fn crawl(fetcher: &Fetcher, urls: &[String], workers: usize) -> CrawlStats {
    let mut by_host: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for url in urls {
        by_host
            .entry(host(url).to_ascii_lowercase())
            .or_default()
            .push(url.clone());
    }
    let mut groups: Vec<Vec<String>> = by_host.into_values().collect();
    groups.sort_by_key(|group| std::cmp::Reverse(group.len()));
    let groups = Mutex::new(groups.into_iter());
    let downloaded = AtomicUsize::new(0);
    let not_modified = AtomicUsize::new(0);
    let failures = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..workers.max(1) {
            scope.spawn(|| {
                loop {
                    let Some(group) = groups.lock().unwrap().next() else {
                        break;
                    };
                    for (index, url) in group.into_iter().enumerate() {
                        if index > 0 {
                            std::thread::sleep(Duration::from_millis(500));
                        }
                        let mut result = fetcher.fetch(&source(&url));
                        if let Err(error) = &result
                            && fetch::is_rate_limited(error)
                        {
                            std::thread::sleep(Duration::from_secs(10));
                            result = fetcher.fetch(&source(&url));
                        }
                        match result {
                            Ok(Outcome::Downloaded(_)) => {
                                downloaded.fetch_add(1, Ordering::Relaxed)
                            }
                            Ok(Outcome::NotModified) => {
                                not_modified.fetch_add(1, Ordering::Relaxed)
                            }
                            Err(error) => {
                                failures.lock().unwrap().push((url, format!("{error:#}")));
                                0
                            }
                        };
                    }
                }
            });
        }
    });
    let mut failures = failures.into_inner().unwrap();
    failures.sort();
    CrawlStats {
        downloaded: downloaded.into_inner(),
        not_modified: not_modified.into_inner(),
        failures,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rfc8805_feeds() {
        let input = "# comment\n\
                     5.101.96.0/21,NL,NL-NH,Amsterdam,1098 XH\n\
                     2001:db8::/32,de,DE-BE,Berlin,\n\
                     192.0.2.0/24,,,,\n\
                     198.51.100.0/24,FR,BOGUS,,  # trailing comment\n\
                     not-a-prefix,FR,,,\n\
                     203.0.113.0/24,FRA,,,\n";
        let (locations, stats) = parse(input.as_bytes()).unwrap();
        assert_eq!(
            stats,
            ParseStats {
                entries: 4,
                invalid: 2
            }
        );
        assert_eq!(locations[0].country.as_deref(), Some("NL"));
        assert_eq!(locations[0].region.as_deref(), Some("NH"));
        assert_eq!(locations[0].city.as_deref(), Some("Amsterdam"));
        assert_eq!(locations[0].postal.as_deref(), Some("1098 XH"));
        assert_eq!(locations[1].country.as_deref(), Some("DE"));
        assert_eq!(locations[1].postal, None);
        assert_eq!(locations[2].country, None);
        assert_eq!(locations[3].region, None);
    }

    #[test]
    fn names_cache_files_and_hosts() {
        assert_eq!(
            cache_name("https://example.com/a.csv"),
            cache_name("https://example.com/a.csv")
        );
        assert_ne!(
            cache_name("https://example.com/a.csv"),
            cache_name("https://example.com/b.csv")
        );
        assert_eq!(host("https://Example.com:8443/a.csv"), "Example.com");
        assert_eq!(host("https://example.com?x"), "example.com");
    }
}
