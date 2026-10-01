use std::collections::HashMap;
use std::io::BufRead;

use anyhow::Result;
use model::{AsName, Asn};

pub fn parse<R: BufRead>(mut reader: R) -> Result<HashMap<Asn, AsName>> {
    let mut names = HashMap::new();
    let mut buffer = Vec::new();
    while reader.read_until(b'\n', &mut buffer)? > 0 {
        let line = String::from_utf8_lossy(&buffer);
        if let Some((asn, name)) = parse_line(line.trim()) {
            names.insert(asn, name);
        }
        buffer.clear();
    }
    Ok(names)
}

fn parse_line(line: &str) -> Option<(Asn, AsName)> {
    let (asn, rest) = line.split_once(' ')?;
    let asn: Asn = asn.parse().ok()?;
    let (rest, country) = match rest.rsplit_once(", ") {
        Some((head, cc)) if cc.len() == 2 && cc.bytes().all(|b| b.is_ascii_uppercase()) => {
            (head, Some(cc.to_string()))
        }
        _ => (rest, None),
    };
    let (handle, organization) = match rest.split_once(" - ") {
        Some((handle, organization)) => (handle.trim(), non_empty(organization)),
        None => (rest.trim(), None),
    };
    Some((
        asn,
        AsName {
            handle: handle.to_string(),
            organization: organization.map(str::to_string),
            country,
        },
    ))
}

fn non_empty(s: &str) -> Option<&str> {
    let s = s.trim();
    (!s.is_empty()).then_some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_ripe_asn_list() {
        let input = "13335 CLOUDFLARENET - Cloudflare, Inc., US\n\
                     3215 FranceTelecom-Orange, FR\n\
                     64512 -Reserved AS-, ZZ\n\
                     garbage\n";
        let names = parse(input.as_bytes()).unwrap();
        assert_eq!(names[&13335].handle, "CLOUDFLARENET");
        assert_eq!(
            names[&13335].organization.as_deref(),
            Some("Cloudflare, Inc.")
        );
        assert_eq!(names[&13335].country.as_deref(), Some("US"));
        assert_eq!(names[&3215].handle, "FranceTelecom-Orange");
        assert_eq!(names[&3215].organization, None);
        assert_eq!(names[&64512].handle, "-Reserved AS-");
        assert_eq!(names.len(), 3);
    }
}
