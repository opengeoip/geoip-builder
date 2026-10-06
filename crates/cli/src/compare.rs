use std::path::Path;

use analysis::compare::{EXAMPLES, Family, Kind, Tally, address, compare, families, top};
use anyhow::Result;
use clap::ValueEnum;
use maxminddb::Reader;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum KindArg {
    Country,
    Asn,
}

impl From<KindArg> for Kind {
    fn from(kind: KindArg) -> Self {
        match kind {
            KindArg::Country => Kind::Country,
            KindArg::Asn => Kind::Asn,
        }
    }
}

fn report(family: &Family, tally: &Tally, limit: usize) {
    let pct = |v: f64| tally.share(v);
    println!("{}, weighted by {}", family.label, family.unit);
    println!("  reference coverage  {:>18.0}", tally.reference());
    println!(
        "  agree               {:>18.0}  {:>6.2} %",
        tally.agree,
        pct(tally.agree)
    );
    println!(
        "  disagree            {:>18.0}  {:>6.2} %",
        tally.disagree,
        pct(tally.disagree)
    );
    println!(
        "  only in reference   {:>18.0}  {:>6.2} %",
        tally.only_reference,
        pct(tally.only_reference)
    );
    println!(
        "  only in ours        {:>18.0}  {:>6.2} % of reference size",
        tally.only_ours,
        pct(tally.only_ours)
    );
    println!("  top disagreements (reference -> ours):");
    for (pair, weight) in top(&tally.pairs, limit) {
        let (reference, ours) = pair;
        println!(
            "    {reference:>10} -> {ours:<10} {weight:>14.0}  {:>6.2} %",
            pct(weight)
        );
        for (_, start, end) in tally.examples(pair).iter().take(EXAMPLES) {
            println!(
                "        {} - {}",
                address(*start, family.v4),
                address(*end, family.v4)
            );
        }
    }
    println!("  top missing:");
    for (key, weight) in top(&tally.missing, limit) {
        println!("    {key:>10} {weight:>14.0}  {:>6.2} %", pct(weight));
    }
    println!("  top extra:");
    for (key, weight) in top(&tally.extra, limit) {
        println!("    {key:>10} {weight:>14.0}  {:>6.2} %", pct(weight));
    }
    println!();
}

pub fn run(
    kind: KindArg,
    ours: &Path,
    reference: &Path,
    limit: usize,
    only: Option<&str>,
) -> Result<()> {
    let ours = Reader::open_readfile(ours)?;
    let reference = Reader::open_readfile(reference)?;
    for family in families() {
        let tally = compare(&ours, &reference, kind.into(), &family, only)?;
        report(&family, &tally, limit);
    }
    Ok(())
}
