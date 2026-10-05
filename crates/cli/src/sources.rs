use std::time::{SystemTime, UNIX_EPOCH};

use fetch::Source;
use model::Registry;

pub const DEFAULT_VRPS_URL: &str = "https://console.rpki-client.org/vrps.json";

pub fn delegated(registry: Registry) -> Source {
    let base = match registry {
        Registry::Afrinic => "https://ftp.afrinic.net/pub/stats/afrinic",
        Registry::Apnic => "https://ftp.apnic.net/stats/apnic",
        Registry::Arin => "https://ftp.arin.net/pub/stats/arin",
        Registry::Lacnic => "https://ftp.lacnic.net/pub/stats/lacnic",
        Registry::RipeNcc => "https://ftp.ripe.net/pub/stats/ripencc",
    };
    Source::new(
        format!("delegated-{registry}"),
        format!("{base}/delegated-{registry}-extended-latest"),
    )
}

pub fn today() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or_default()
}

pub fn atlas() -> Source {
    let (year, month, day) = src_atlas::civil_from_days(today() as i64 - 1);
    Source::new(
        "atlas-probes.json.bz2",
        src_atlas::archive_url(year, month, day),
    )
}

pub fn asnames() -> Source {
    Source::new("asn.txt", "https://ftp.ripe.net/ripe/asnames/asn.txt")
}

pub fn arin_geofeed_inetnums() -> Source {
    Source::new(
        "arin-geofeed-inetnums.json",
        "https://geofeeds.packetvis.com/geolocatemuch/arin.inetnums",
    )
}

pub fn violating_probes_index() -> Source {
    Source::new(
        "violating-probes-index.json",
        "https://api.github.com/repos/kizhikevich/violating_ripe_probes/contents",
    )
}

pub fn violating_probes(file: &str) -> Source {
    Source::new(
        "violating-probes.txt",
        format!("https://raw.githubusercontent.com/kizhikevich/violating_ripe_probes/main/{file}"),
    )
}

pub fn anycast(version: u8) -> Source {
    Source::new(
        format!("anycast-ipv{version}.csv"),
        format!(
            "https://raw.githubusercontent.com/ut-dacs/anycast-census/main/IPv{version}-latest.csv"
        ),
    )
}

pub fn vrps(url: &str) -> Source {
    Source::new("vrps.json", url)
}

pub fn ris(collector: &str) -> Source {
    Source::new(
        format!("ris-{collector}.bview.gz"),
        format!("https://data.ris.ripe.net/{collector}/latest-bview.gz"),
    )
}

pub fn rpsl() -> Vec<(Source, Registry)> {
    [
        (
            "rpsl-ripe-inetnum.gz",
            "https://ftp.ripe.net/ripe/dbase/split/ripe.db.inetnum.gz",
            Registry::RipeNcc,
        ),
        (
            "rpsl-ripe-inet6num.gz",
            "https://ftp.ripe.net/ripe/dbase/split/ripe.db.inet6num.gz",
            Registry::RipeNcc,
        ),
        (
            "rpsl-apnic-inetnum.gz",
            "https://ftp.apnic.net/apnic/whois/apnic.db.inetnum.gz",
            Registry::Apnic,
        ),
        (
            "rpsl-apnic-inet6num.gz",
            "https://ftp.apnic.net/apnic/whois/apnic.db.inet6num.gz",
            Registry::Apnic,
        ),
        (
            "rpsl-afrinic.gz",
            "https://ftp.afrinic.net/dbase/afrinic.db.gz",
            Registry::Afrinic,
        ),
        (
            "rpsl-lacnic.gz",
            "https://ftp.lacnic.net/lacnic/dbase/lacnic.db.gz",
            Registry::Lacnic,
        ),
    ]
    .into_iter()
    .map(|(name, url, registry)| (Source::new(name, url), registry))
    .collect()
}

pub fn all(collectors: &[String], vrps_url: &str) -> Vec<Source> {
    let mut sources: Vec<Source> = Registry::ALL.into_iter().map(delegated).collect();
    sources.push(asnames());
    sources.push(vrps(vrps_url));
    sources.extend(collectors.iter().map(|c| ris(c)));
    sources.extend(rpsl().into_iter().map(|(source, _)| source));
    sources.push(arin_geofeed_inetnums());
    sources.push(atlas());
    sources.push(anycast(4));
    sources.push(anycast(6));
    sources.push(violating_probes_index());
    sources
}
