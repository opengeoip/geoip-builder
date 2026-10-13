use std::fs;
use std::net::IpAddr;
use std::path::Path;

use analysis::audit::{Rng, estimate, ipv4_ranges, sample, size};
use anyhow::{Context, Result};
use maxminddb::{Reader, path};
use serde_json::Value;

const HEADER: [&str; 11] = [
    "address",
    "flagged",
    "network",
    "asn",
    "organization",
    "peeringdb_types",
    "apnic_users",
    "flagged_addresses",
    "unflagged_addresses",
    "verdict",
    "evidence",
];

pub struct SampleOptions<'a> {
    pub data_dir: &'a Path,
    pub asn: &'a Path,
    pub hosting: &'a Path,
    pub per_side: usize,
    pub seed: u64,
    pub output: &'a Path,
}

pub fn sample_run(options: &SampleOptions<'_>) -> Result<()> {
    let asn = Reader::open_readfile(options.asn)
        .with_context(|| format!("opening {}, run build first", options.asn.display()))?;
    let hosting = Reader::open_readfile(options.hosting)?;
    let routed = ipv4_ranges(&asn, false)?;
    let flagged_addresses = size(&ipv4_ranges(&hosting, true)?);
    let unflagged_addresses = size(&routed) - flagged_addresses;
    let networks = pipeline::candidates::networks(options.data_dir)?;
    let users = pipeline::build::apnic_users(options.data_dir)?;
    let draws = sample(
        &routed,
        &hosting,
        options.per_side,
        &mut Rng::new(options.seed),
    )?;
    if let Some(parent) = options.output.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = csv::Writer::from_path(options.output)?;
    out.write_record(HEADER)?;
    for draw in &draws {
        let result = asn.lookup(IpAddr::V4(draw.address))?;
        let network = result.network().map(|n| n.to_string()).unwrap_or_default();
        let number = result.decode_path::<u32>(&path!["autonomous_system_number"])?;
        let organization = result
            .decode_path::<String>(&path!["autonomous_system_organization"])?
            .unwrap_or_default();
        let types = number
            .and_then(|n| networks.get(&n))
            .map(|n| n.types().join(" + "))
            .unwrap_or_default();
        let apnic = number
            .and_then(|n| users.get(&n))
            .map(u64::to_string)
            .unwrap_or_default();
        out.write_record([
            draw.address.to_string(),
            draw.flagged.to_string(),
            network,
            number.map(|n| n.to_string()).unwrap_or_default(),
            organization,
            types,
            apnic,
            flagged_addresses.to_string(),
            unflagged_addresses.to_string(),
            String::new(),
            String::new(),
        ])?;
    }
    out.flush()?;
    println!(
        "wrote {} addresses to {} ({} flagged and {} unflagged IPv4 addresses routed)",
        draws.len(),
        options.output.display(),
        flagged_addresses,
        unflagged_addresses
    );
    Ok(())
}

fn verdict(value: &str) -> Option<bool> {
    match value.trim() {
        "hosting" => Some(true),
        "not-hosting" => Some(false),
        _ => None,
    }
}

pub fn score_run(audit: &Path, json: Option<&Path>) -> Result<()> {
    let mut reader = csv::Reader::from_path(audit)?;
    let headers = reader.headers()?.clone();
    let column = |name: &str| {
        headers
            .iter()
            .position(|h| h == name)
            .with_context(|| format!("no {name} column in {}", audit.display()))
    };
    let (flagged, verdict_column) = (column("flagged")?, column("verdict")?);
    let (flagged_space, unflagged_space) =
        (column("flagged_addresses")?, column("unflagged_addresses")?);
    let mut verdicts = Vec::new();
    let mut spaces = (0, 0);
    for record in reader.records() {
        let record = record?;
        verdicts.push((&record[flagged] == "true", verdict(&record[verdict_column])));
        spaces = (
            record[flagged_space].parse()?,
            record[unflagged_space].parse()?,
        );
    }
    let estimate = estimate(&verdicts, spaces.0, spaces.1);
    let line = |label: &str, (p, low, high): (f64, f64, f64), n: usize| {
        println!(
            "  {label:<34} {:>6.1} %   95 % interval {:>5.1} - {:>5.1} %   ({n} judged)",
            100.0 * p,
            100.0 * low,
            100.0 * high
        );
    };
    println!("IPv4 hosting audit, weighted by addresses:");
    line(
        "flagged and hosting",
        estimate.precision(),
        estimate.flagged_judged,
    );
    line(
        "not flagged and not hosting",
        estimate.negative_precision(),
        estimate.unflagged_judged,
    );
    println!(
        "  hosting addresses flagged          {:>6.1} %   ({:.0} flagged, {:.0} missed)",
        100.0 * estimate.recall(),
        estimate.hosting_addresses_flagged,
        estimate.hosting_addresses_missed
    );
    println!("  without a verdict                  {}", estimate.unknown);
    if let Some(path) = json {
        let (precision, precision_low, precision_high) = estimate.precision();
        let (negative, negative_low, negative_high) = estimate.negative_precision();
        let value: Value = serde_json::json!({
            "precision": precision,
            "precision_interval": [precision_low, precision_high],
            "negative_precision": negative,
            "negative_precision_interval": [negative_low, negative_high],
            "recall": estimate.recall(),
            "flagged_judged": estimate.flagged_judged,
            "unflagged_judged": estimate.unflagged_judged,
            "unknown": estimate.unknown,
        });
        fs::write(path, serde_json::to_string_pretty(&value)?)?;
    }
    Ok(())
}
