use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter};
use std::path::Path;

use anyhow::{Context, Result};
use flate2::read::MultiGzDecoder;
use maxminddb::Reader;
use mmdb_writer::Writer;

pub fn open(dir: &Path, source: &fetch::Source) -> Result<Box<dyn BufRead>> {
    let path = source.path(dir);
    let file = File::open(&path)
        .with_context(|| format!("opening {}, run fetch first", path.display()))?;
    let reader = BufReader::with_capacity(1 << 20, file);
    Ok(if source.name.ends_with(".gz") {
        Box::new(BufReader::with_capacity(
            1 << 20,
            MultiGzDecoder::new(reader),
        ))
    } else {
        Box::new(reader)
    })
}

pub fn write(writer: Writer, path: &Path) -> Result<u64> {
    let partial = path.with_extension("mmdb.part");
    writer.write_to(BufWriter::new(File::create(&partial)?))?;
    fs::rename(&partial, path)?;
    Ok(fs::metadata(path)?.len())
}

pub fn in_memory(writer: Writer) -> Result<Reader<Vec<u8>>> {
    let mut bytes = Vec::new();
    writer.write_to(&mut bytes)?;
    Ok(Reader::from_source(bytes)?)
}
