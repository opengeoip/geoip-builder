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

pub fn all(collectors: &[String], vrps_url: &str) -> Vec<Source> {
    let mut sources: Vec<Source> = Registry::ALL.into_iter().map(delegated).collect();
    sources.push(asnames());
    sources.push(vrps(vrps_url));
    sources.extend(collectors.iter().map(|c| ris(c)));
    sources
}
