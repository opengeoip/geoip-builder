use std::fs;
use std::path::Path;

use anyhow::Result;
use pipeline::candidates::{Options, Report, add_to_catalog, candidates};

fn write(report: &Report, output: &Path) -> Result<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = csv::Writer::from_path(output)?;
    out.write_record([
        "asn",
        "name",
        "peeringdb_type",
        "website",
        "wrong_probes",
        "probes",
        "top_error",
        "ipv4_share_without_geofeed",
        "found_geofeed",
    ])?;
    for row in &report.rows {
        out.write_record([
            row.asn.to_string(),
            row.name.clone(),
            row.peeringdb_type.clone(),
            row.website.clone(),
            row.candidate.wrong.to_string(),
            row.candidate.probes.to_string(),
            row.candidate.top_error(),
            row.ipv4_share_without_geofeed
                .map(|share| format!("{share:.4}"))
                .unwrap_or_default(),
            row.found_geofeed.clone(),
        ])?;
    }
    out.flush()?;
    Ok(())
}

pub fn run(options: &Options<'_>, output: &Path, add_to: Option<&Path>) -> Result<()> {
    let report = candidates(options)?;
    if report.coverage_missing {
        eprintln!(
            "{} not found, run coverage for the share without geofeed",
            options.coverage.display()
        );
    }
    write(&report, output)?;
    eprintln!(
        "candidates: {} ASes with misplaced probes, written to {}",
        report.rows.len(),
        output.display()
    );
    for (asn, url) in &report.new_geofeeds {
        eprintln!("  AS{asn}: new geofeed {url}");
    }
    if let Some(manual) = add_to
        && !report.new_geofeeds.is_empty()
    {
        let count = add_to_catalog(manual, report.new_geofeeds)?;
        eprintln!("{}: {count} rows", manual.display());
    }
    Ok(())
}
