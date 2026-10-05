use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use anyhow::Result;
use maxminddb::{Reader, path};
use src_atlas::Probe;

use crate::truth;

struct Database {
    name: String,
    reader: Reader<Vec<u8>>,
}

pub fn country(reader: &Reader<Vec<u8>>, probe: &Probe) -> Result<Option<String>> {
    Ok(reader
        .lookup(probe.address)?
        .decode_path::<String>(&path!["country", "iso_code"])?)
}

#[derive(Default)]
struct Score {
    correct: usize,
    wrong: usize,
    missing: usize,
    errors: HashMap<(String, String), Vec<u32>>,
}

fn pct(part: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        100.0 * part as f64 / total as f64
    }
}

fn report(
    label: &str,
    probes: &[&Probe],
    answers: &[Vec<Option<String>>],
    databases: &[Database],
    top: usize,
) {
    println!("{label}: {} probe addresses", probes.len());
    for (db, answers) in databases.iter().zip(answers) {
        let mut score = Score::default();
        for (probe, answer) in probes.iter().zip(answers) {
            match answer {
                Some(answer) if *answer == probe.country => score.correct += 1,
                Some(answer) => {
                    score.wrong += 1;
                    score
                        .errors
                        .entry((probe.country.clone(), answer.clone()))
                        .or_default()
                        .push(probe.id);
                }
                None => score.missing += 1,
            }
        }
        let total = probes.len();
        println!(
            "  {:<24} correct {:>6} {:>6.2} %   wrong {:>5} {:>6.2} %   missing {:>4} {:>5.2} %",
            db.name,
            score.correct,
            pct(score.correct, total),
            score.wrong,
            pct(score.wrong, total),
            score.missing,
            pct(score.missing, total)
        );
        let mut errors: Vec<_> = score.errors.into_iter().collect();
        errors.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
        for ((truth, answer), ids) in errors.iter().take(top) {
            let sample: Vec<String> = ids.iter().take(5).map(u32::to_string).collect();
            println!(
                "      {truth} -> {answer:<4} {:>4}  probes {}",
                ids.len(),
                sample.join(", ")
            );
        }
    }
    if let [a, b, ..] = databases {
        let (mut both, mut only_a, mut only_b, mut neither) = (0, 0, 0, 0);
        let mut cases: Vec<String> = Vec::new();
        for (index, probe) in probes.iter().enumerate() {
            let right = |answers: &Vec<Option<String>>| {
                answers[index].as_deref() == Some(probe.country.as_str())
            };
            match (right(&answers[0]), right(&answers[1])) {
                (true, true) => both += 1,
                (true, false) => only_a += 1,
                (false, true) => {
                    only_b += 1;
                    cases.push(format!(
                        "      probe {:<6} {:<40} truth {}  {} {}  {} {}",
                        probe.id,
                        probe.address,
                        probe.country,
                        a.name,
                        answers[0][index].as_deref().unwrap_or("-"),
                        b.name,
                        answers[1][index].as_deref().unwrap_or("-")
                    ));
                }
                (false, false) => neither += 1,
            }
        }
        println!(
            "  head to head: both right {both}, only {} right {only_a}, only {} right {only_b}, both wrong {neither}",
            a.name, b.name
        );
        if !cases.is_empty() {
            println!("    where only {} is right:", b.name);
            for case in cases.iter().take(top) {
                println!("{case}");
            }
        }
    }
    println!();
}

pub struct Options<'a> {
    pub data_dir: &'a Path,
    pub truth: &'a Path,
    pub only: Option<&'a str>,
    pub keep_suspicious: bool,
    pub top: usize,
}

pub fn run(options: &Options<'_>, paths: &[PathBuf]) -> Result<()> {
    let probes = src_atlas::parse(BufReader::new(File::open(options.truth)?))?;
    let databases: Vec<Database> = paths
        .iter()
        .map(|path| {
            Ok(Database {
                name: path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                reader: Reader::open_readfile(path)?,
            })
        })
        .collect::<Result<_>>()?;
    let probes: Vec<Probe> = probes
        .into_iter()
        .filter(|p| options.only.is_none_or(|c| p.country == c))
        .collect();
    let probes = if options.keep_suspicious {
        probes
    } else {
        let (kept, excluded) = truth::reliable(options.data_dir, probes)?;
        println!(
            "excluded {} addresses of probes listed as misplaced ({} listed) and {} anycast addresses",
            excluded.misplaced, excluded.listed, excluded.anycast
        );
        println!();
        kept
    };
    let selected: Vec<&Probe> = probes.iter().collect();
    for (label, v4) in [("IPv4", true), ("IPv6", false)] {
        for (group, filter) in [
            ("", None),
            (" anchors", Some(true)),
            (" probes", Some(false)),
        ] {
            let family: Vec<&Probe> = selected
                .iter()
                .copied()
                .filter(|p| p.address.is_ipv4() == v4 && filter.is_none_or(|a| p.is_anchor == a))
                .collect();
            let answers: Vec<Vec<Option<String>>> = databases
                .iter()
                .map(|db| {
                    family
                        .iter()
                        .map(|p| country(&db.reader, p))
                        .collect::<Result<_>>()
                })
                .collect::<Result<_>>()?;
            let top = if filter.is_none() { options.top } else { 0 };
            report(
                &format!("{label}{group}"),
                &family,
                &answers,
                &databases,
                top,
            );
        }
    }
    Ok(())
}
