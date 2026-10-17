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
    let mut reader = BufReader::with_capacity(1 << 20, file);
    // Check the gzip magic rather than the name: servers that send
    // Content-Encoding: gzip get decompressed by ureq on download.
    Ok(if reader.fill_buf()?.starts_with(&[0x1f, 0x8b]) {
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

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use flate2::Compression;
    use flate2::write::GzEncoder;

    use super::*;

    fn read(dir: &Path, name: &str, bytes: &[u8]) -> String {
        fs::write(dir.join(name), bytes).unwrap();
        let mut text = String::new();
        open(dir, &fetch::Source::new(name, ""))
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        text
    }

    #[test]
    fn detects_gzip_by_content() {
        let dir = tempfile::tempdir().unwrap();
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(b"prefix\n1.1.1.0/24\n").unwrap();
        let gzip = encoder.finish().unwrap();
        assert_eq!(read(dir.path(), "a.csv", &gzip), "prefix\n1.1.1.0/24\n");
        assert_eq!(
            read(dir.path(), "b.csv.gz", b"prefix\n1.1.1.0/24\n"),
            "prefix\n1.1.1.0/24\n"
        );
    }
}
