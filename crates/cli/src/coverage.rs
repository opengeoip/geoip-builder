use std::fs;
use std::path::Path;

use anyhow::Result;
use pipeline::build::Inputs;
use pipeline::coverage::coverage;

use crate::log;

pub fn run(inputs: &Inputs<'_>, output: &Path, top: usize) -> Result<()> {
    let coverage = coverage(inputs, &mut log)?;
    let label = |asn: u32| {
        coverage
            .names
            .get(&asn)
            .map(|n| n.organization.as_deref().unwrap_or(&n.handle))
            .unwrap_or_default()
    };

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = csv::Writer::from_path(output)?;
    out.write_record([
        "asn",
        "name",
        "country",
        "ipv4_announced",
        "ipv4_without_geofeed",
        "ipv4_share_without_geofeed",
        "ipv6_announced_48",
        "ipv6_without_geofeed_48",
    ])?;
    for row in &coverage.rows {
        let country = coverage
            .names
            .get(&row.asn)
            .and_then(|n| n.country.as_deref())
            .unwrap_or_default();
        out.write_record([
            row.asn.to_string(),
            label(row.asn).to_string(),
            country.to_string(),
            format!("{:.0}", row.ipv4.announced),
            format!("{:.0}", row.ipv4.uncovered),
            format!("{:.4}", row.ipv4.share_uncovered()),
            format!("{:.0}", row.ipv6.announced),
            format!("{:.0}", row.ipv6.uncovered),
        ])?;
    }
    out.flush()?;

    eprintln!(
        "coverage: {} ASes, {:.1} % of announced IPv4 space without a geofeed, written to {}",
        coverage.rows.len(),
        analysis::coverage::ipv4_share_uncovered(&coverage.rows),
        output.display()
    );
    for row in coverage.rows.iter().take(top) {
        eprintln!(
            "  AS{:<10} {:>12.0} IPv4 addresses without a geofeed ({:>5.1} % of its space)  {}",
            row.asn,
            row.ipv4.uncovered,
            100.0 * row.ipv4.uncovered / row.ipv4.announced.max(1.0),
            label(row.asn)
        );
    }
    Ok(())
}
