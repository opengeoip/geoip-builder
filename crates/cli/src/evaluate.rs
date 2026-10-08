use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

use analysis::evaluate::{
    Bucket, Network, answers, by_continent, by_network, groups, head_to_head, percent, score,
};
use anyhow::Result;
use maxminddb::Reader;
use serde_json::{Value, json};
use src_atlas::Probe;

struct Database {
    name: String,
    reader: Reader<Vec<u8>>,
}

pub struct Options<'a> {
    pub data_dir: &'a Path,
    pub truth: &'a Path,
    pub only: Option<&'a str>,
    pub keep_suspicious: bool,
    pub top: usize,
    pub json: Option<&'a Path>,
}

fn scores(
    family: &str,
    by: Option<&str>,
    group: &str,
    probes: &[&Probe],
    answers: &[Vec<Option<String>>],
    databases: &[Database],
) -> Value {
    let total = probes.len();
    let databases: Vec<Value> = databases
        .iter()
        .zip(answers)
        .map(|(db, answers)| {
            let score = score(probes, answers);
            json!({
                "name": db.name,
                "correct": score.correct,
                "wrong": score.wrong,
                "missing": score.missing,
                "accuracy": (percent(score.correct, total) * 100.0).round() / 100.0,
            })
        })
        .collect();
    json!({
        "family": family,
        "by": by,
        "group": group,
        "probes": total,
        "databases": databases,
    })
}

fn breakdown(
    family: &str,
    by: &str,
    buckets: &[Bucket<'_>],
    databases: &[Database],
) -> Result<Vec<Value>> {
    println!("{family} by {by}:");
    let mut results = Vec::new();
    for bucket in buckets {
        let answers: Vec<Vec<Option<String>>> = databases
            .iter()
            .map(|db| answers(&db.reader, &bucket.probes))
            .collect::<Result<_>>()?;
        let total = bucket.probes.len();
        let columns: Vec<String> = databases
            .iter()
            .zip(&answers)
            .map(|(db, answers)| {
                let correct = score(&bucket.probes, answers).correct;
                format!("{} {:>6.2} %", db.name, percent(correct, total))
            })
            .collect();
        println!(
            "  {:<16} {:>6}   {}",
            bucket.label,
            total,
            columns.join("   ")
        );
        results.push(scores(
            family,
            Some(by),
            bucket.label,
            &bucket.probes,
            &answers,
            databases,
        ));
    }
    println!();
    Ok(results)
}

fn report(
    label: &str,
    probes: &[&Probe],
    answers: &[Vec<Option<String>>],
    databases: &[Database],
    top: usize,
) {
    println!("{label}: {} probe addresses", probes.len());
    let total = probes.len();
    for (db, answers) in databases.iter().zip(answers) {
        let score = score(probes, answers);
        println!(
            "  {:<24} correct {:>6} {:>6.2} %   wrong {:>5} {:>6.2} %   missing {:>4} {:>5.2} %",
            db.name,
            score.correct,
            percent(score.correct, total),
            score.wrong,
            percent(score.wrong, total),
            score.missing,
            percent(score.missing, total)
        );
        for ((truth, answer), ids) in score.errors.iter().take(top) {
            let sample: Vec<String> = ids.iter().take(5).map(u32::to_string).collect();
            println!(
                "      {truth} -> {answer:<4} {:>4}  probes {}",
                ids.len(),
                sample.join(", ")
            );
        }
    }
    if let [a, b, ..] = databases {
        let duel = head_to_head(probes, &answers[0], &answers[1]);
        println!(
            "  head to head: both right {}, only {} right {}, only {} right {}, both wrong {}",
            duel.both, a.name, duel.only_first, b.name, duel.only_second, duel.neither
        );
        if !duel.only_second_right.is_empty() {
            println!("    where only {} is right:", b.name);
            for &index in duel.only_second_right.iter().take(top) {
                let probe = probes[index];
                println!(
                    "      probe {:<6} {:<40} truth {}  {} {}  {} {}",
                    probe.id,
                    probe.address,
                    probe.country,
                    a.name,
                    answers[0][index].as_deref().unwrap_or("-"),
                    b.name,
                    answers[1][index].as_deref().unwrap_or("-")
                );
            }
        }
    }
    println!();
}

fn open(path: &Path) -> Result<Database> {
    Ok(Database {
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        reader: Reader::open_readfile(path)?,
    })
}

pub fn run(options: &Options<'_>, paths: &[PathBuf]) -> Result<()> {
    let probes = src_atlas::parse(BufReader::new(File::open(options.truth)?))?;
    let databases: Vec<Database> = paths.iter().map(|path| open(path)).collect::<Result<_>>()?;
    let probes: Vec<Probe> = probes
        .into_iter()
        .filter(|p| options.only.is_none_or(|c| p.country == c))
        .collect();
    let probes = if options.keep_suspicious {
        probes
    } else {
        let (kept, excluded) = pipeline::truth::reliable(options.data_dir, probes)?;
        println!(
            "excluded {} addresses of probes listed as misplaced ({} listed), {} anycast addresses, {} addresses of probes placed by IP geolocation and {} addresses without an AS",
            excluded.misplaced,
            excluded.listed,
            excluded.anycast,
            excluded.auto_located,
            excluded.unrouted
        );
        println!();
        kept
    };
    let networks: HashMap<u32, Network> = pipeline::candidates::networks(options.data_dir)?
        .into_iter()
        .map(|(asn, n)| (asn, Network::from_peeringdb(n.info_type.as_deref())))
        .collect();
    let mut results = Vec::new();
    for group in groups() {
        let selected = group.select(&probes);
        let answers: Vec<Vec<Option<String>>> = databases
            .iter()
            .map(|db| answers(&db.reader, &selected))
            .collect::<Result<_>>()?;
        let top = if group.is_detailed() { options.top } else { 0 };
        report(&group.label, &selected, &answers, &databases, top);
        let family = if group.ipv4 { "IPv4" } else { "IPv6" };
        let name = match group.anchors {
            None => "all",
            Some(true) => "anchors",
            Some(false) => "probes",
        };
        results.push(scores(family, None, name, &selected, &answers, &databases));
    }
    for (family, ipv4) in [("IPv4", true), ("IPv6", false)] {
        let selected: Vec<&Probe> = probes
            .iter()
            .filter(|p| p.address.is_ipv4() == ipv4)
            .collect();
        results.extend(breakdown(
            family,
            "network",
            &by_network(&selected, &networks),
            &databases,
        )?);
        results.extend(breakdown(
            family,
            "continent",
            &by_continent(&selected),
            &databases,
        )?);
    }
    if let Some(path) = options.json {
        serde_json::to_writer_pretty(
            BufWriter::new(File::create(path)?),
            &json!({ "results": results }),
        )?;
    }
    Ok(())
}
