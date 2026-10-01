use fetch::Source;
use model::Registry;
use src_geofeed::Seed;

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

pub fn asnames() -> Source {
    Source::new("asn.txt", "https://ftp.ripe.net/ripe/asnames/asn.txt")
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

const GOOGLE: &[u32] = &[
    15169, 19527, 36040, 36383, 36384, 36411, 41264, 43515, 45566, 139070, 139190, 395973, 396982,
];

pub const SEEDS: &[Seed] = &[
    Seed {
        url: "https://ip-ranges.amazonaws.com/geo-ip-feed.csv",
        asns: &[7224, 8987, 14618, 16509],
    },
    Seed {
        url: "https://www.gstatic.com/ipranges/cloud_geofeed",
        asns: GOOGLE,
    },
    Seed {
        url: "https://www.gstatic.com/geofeed/corp_external",
        asns: GOOGLE,
    },
    Seed {
        url: "https://api.cloudflare.com/local-ip-ranges.csv",
        asns: &[13335, 14789, 209242],
    },
    Seed {
        url: "https://geoip.linode.com/",
        asns: &[20940, 63949],
    },
    Seed {
        url: "https://www.digitalocean.com/geo/google.csv",
        asns: &[14061],
    },
];

pub fn all(collectors: &[String], vrps_url: &str) -> Vec<Source> {
    let mut sources: Vec<Source> = Registry::ALL.into_iter().map(delegated).collect();
    sources.push(asnames());
    sources.push(vrps(vrps_url));
    sources.extend(collectors.iter().map(|c| ris(c)));
    sources.extend(rpsl().into_iter().map(|(source, _)| source));
    sources
}
