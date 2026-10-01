use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use mmdb_writer::Writer;
use model::Registry;
use src_bgp::RibCollector;
use src_rpki::Validator;

use crate::sources;

fn open(dir: &Path, source: &fetch::Source) -> Result<BufReader<File>> {
    let path = source.path(dir);
    let file = File::open(&path)
        .with_context(|| format!("opening {}, run fetch first", path.display()))?;
    Ok(BufReader::with_capacity(1 << 20, file))
}

fn write(writer: Writer, path: &Path) -> Result<()> {
    let partial = path.with_extension("mmdb.part");
    writer.write_to(BufWriter::new(File::create(&partial)?))?;
    fs::rename(&partial, path)?;
    eprintln!(
        "wrote {} ({} bytes)",
        path.display(),
        fs::metadata(path)?.len()
    );
    Ok(())
}

pub fn run(data_dir: &Path, out_dir: &Path, collectors: &[String], vrps_url: &str) -> Result<()> {
    fs::create_dir_all(out_dir)?;
    let epoch = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

    let started = Instant::now();
    let mut delegations = Vec::new();
    for registry in Registry::ALL {
        let parsed = src_delegated::parse(open(data_dir, &sources::delegated(registry))?)
            .with_context(|| format!("parsing delegated-{registry}"))?;
        eprintln!("delegated-{registry}: {} networks", parsed.len());
        delegations.extend(parsed);
    }
    let (country, stats) = merge::country_db(delegations, epoch);
    eprintln!(
        "country: {} networks in {:.1?}",
        stats.networks,
        started.elapsed()
    );
    write(country, &out_dir.join("country.mmdb"))?;

    let started = Instant::now();
    let names = src_asnames::parse(open(data_dir, &sources::asnames())?)?;
    eprintln!("asn.txt: {} names", names.len());
    let vrps = src_rpki::parse(open(data_dir, &sources::vrps(vrps_url))?)?;
    eprintln!("vrps.json: {} VRPs", vrps.len());
    let validator = Validator::new(&vrps);
    drop(vrps);
    let mut rib = RibCollector::default();
    for collector in collectors {
        let source = sources::ris(collector);
        rib.add_file(&source.path(data_dir))?;
        eprintln!("{}: parsed after {:.1?}", source.name, started.elapsed());
    }
    let routes = rib.into_routes();
    let (asn, stats) = merge::asn_db(&routes, &validator, &names, epoch);
    eprintln!(
        "asn: {} routes, {} valid, {} invalid, {} not found, {} valid without a name, in {:.1?}",
        stats.routes,
        stats.valid,
        stats.invalid,
        stats.not_found,
        stats.unnamed,
        started.elapsed()
    );
    write(asn, &out_dir.join("asn.mmdb"))?;
    Ok(())
}
