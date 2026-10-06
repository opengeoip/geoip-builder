use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use analysis::evaluate::{answers, groups, head_to_head, percent, score};
use anyhow::Result;
use maxminddb::Reader;
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
            "excluded {} addresses of probes listed as misplaced ({} listed) and {} anycast addresses",
            excluded.misplaced, excluded.listed, excluded.anycast
        );
        println!();
        kept
    };
    for group in groups() {
        let selected = group.select(&probes);
        let answers: Vec<Vec<Option<String>>> = databases
            .iter()
            .map(|db| answers(&db.reader, &selected))
            .collect::<Result<_>>()?;
        let top = if group.is_detailed() { options.top } else { 0 };
        report(&group.label, &selected, &answers, &databases, top);
    }
    Ok(())
}
