use std::fmt;

use crate::continent::Continent;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Country(u8);

impl Country {
    pub fn parse(code: &str) -> Option<Country> {
        let code = code.trim().to_ascii_uppercase();
        let code = if code == "UK" { "GB" } else { &code };
        COUNTRIES
            .binary_search_by(|(entry, _, _)| (*entry).cmp(code))
            .ok()
            .and_then(|index| u8::try_from(index).ok())
            .map(Country)
    }

    pub fn as_str(self) -> &'static str {
        COUNTRIES[usize::from(self.0)].0
    }

    pub fn continent(self) -> Continent {
        COUNTRIES[usize::from(self.0)].1
    }

    pub fn name(self) -> &'static str {
        COUNTRIES[usize::from(self.0)].2
    }

    pub fn is_in_european_union(self) -> bool {
        EUROPEAN_UNION.contains(&self.as_str())
    }
}

impl fmt::Display for Country {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for Country {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

const EUROPEAN_UNION: [&str; 27] = [
    "AT", "BE", "BG", "CY", "CZ", "DE", "DK", "EE", "ES", "FI", "FR", "GR", "HR", "HU", "IE", "IT",
    "LT", "LU", "LV", "MT", "NL", "PL", "PT", "RO", "SE", "SI", "SK",
];

const COUNTRIES: [(&str, Continent, &str); 250] = [
    ("AD", Continent::Europe, "Andorra"),
    ("AE", Continent::Asia, "United Arab Emirates"),
    ("AF", Continent::Asia, "Afghanistan"),
    ("AG", Continent::NorthAmerica, "Antigua and Barbuda"),
    ("AI", Continent::NorthAmerica, "Anguilla"),
    ("AL", Continent::Europe, "Albania"),
    ("AM", Continent::Asia, "Armenia"),
    ("AO", Continent::Africa, "Angola"),
    ("AQ", Continent::Antarctica, "Antarctica"),
    ("AR", Continent::SouthAmerica, "Argentina"),
    ("AS", Continent::Oceania, "American Samoa"),
    ("AT", Continent::Europe, "Austria"),
    ("AU", Continent::Oceania, "Australia"),
    ("AW", Continent::NorthAmerica, "Aruba"),
    ("AX", Continent::Europe, "Åland Islands"),
    ("AZ", Continent::Asia, "Azerbaijan"),
    ("BA", Continent::Europe, "Bosnia and Herzegovina"),
    ("BB", Continent::NorthAmerica, "Barbados"),
    ("BD", Continent::Asia, "Bangladesh"),
    ("BE", Continent::Europe, "Belgium"),
    ("BF", Continent::Africa, "Burkina Faso"),
    ("BG", Continent::Europe, "Bulgaria"),
    ("BH", Continent::Asia, "Bahrain"),
    ("BI", Continent::Africa, "Burundi"),
    ("BJ", Continent::Africa, "Benin"),
    ("BL", Continent::NorthAmerica, "Saint Barthélemy"),
    ("BM", Continent::NorthAmerica, "Bermuda"),
    ("BN", Continent::Asia, "Brunei"),
    ("BO", Continent::SouthAmerica, "Bolivia"),
    (
        "BQ",
        Continent::NorthAmerica,
        "Bonaire, Sint Eustatius, and Saba",
    ),
    ("BR", Continent::SouthAmerica, "Brazil"),
    ("BS", Continent::NorthAmerica, "Bahamas"),
    ("BT", Continent::Asia, "Bhutan"),
    ("BV", Continent::Antarctica, "Bouvet Island"),
    ("BW", Continent::Africa, "Botswana"),
    ("BY", Continent::Europe, "Belarus"),
    ("BZ", Continent::NorthAmerica, "Belize"),
    ("CA", Continent::NorthAmerica, "Canada"),
    ("CC", Continent::Asia, "Cocos (Keeling) Islands"),
    ("CD", Continent::Africa, "DR Congo"),
    ("CF", Continent::Africa, "Central African Republic"),
    ("CG", Continent::Africa, "Congo Republic"),
    ("CH", Continent::Europe, "Switzerland"),
    ("CI", Continent::Africa, "Ivory Coast"),
    ("CK", Continent::Oceania, "Cook Islands"),
    ("CL", Continent::SouthAmerica, "Chile"),
    ("CM", Continent::Africa, "Cameroon"),
    ("CN", Continent::Asia, "China"),
    ("CO", Continent::SouthAmerica, "Colombia"),
    ("CR", Continent::NorthAmerica, "Costa Rica"),
    ("CU", Continent::NorthAmerica, "Cuba"),
    ("CV", Continent::Africa, "Cabo Verde"),
    ("CW", Continent::NorthAmerica, "Curaçao"),
    ("CX", Continent::Oceania, "Christmas Island"),
    ("CY", Continent::Europe, "Cyprus"),
    ("CZ", Continent::Europe, "Czechia"),
    ("DE", Continent::Europe, "Germany"),
    ("DJ", Continent::Africa, "Djibouti"),
    ("DK", Continent::Europe, "Denmark"),
    ("DM", Continent::NorthAmerica, "Dominica"),
    ("DO", Continent::NorthAmerica, "Dominican Republic"),
    ("DZ", Continent::Africa, "Algeria"),
    ("EC", Continent::SouthAmerica, "Ecuador"),
    ("EE", Continent::Europe, "Estonia"),
    ("EG", Continent::Africa, "Egypt"),
    ("EH", Continent::Africa, "Western Sahara"),
    ("ER", Continent::Africa, "Eritrea"),
    ("ES", Continent::Europe, "Spain"),
    ("ET", Continent::Africa, "Ethiopia"),
    ("FI", Continent::Europe, "Finland"),
    ("FJ", Continent::Oceania, "Fiji"),
    ("FK", Continent::SouthAmerica, "Falkland Islands"),
    ("FM", Continent::Oceania, "Federated States of Micronesia"),
    ("FO", Continent::Europe, "Faroe Islands"),
    ("FR", Continent::Europe, "France"),
    ("GA", Continent::Africa, "Gabon"),
    ("GB", Continent::Europe, "United Kingdom"),
    ("GD", Continent::NorthAmerica, "Grenada"),
    ("GE", Continent::Asia, "Georgia"),
    ("GF", Continent::SouthAmerica, "French Guiana"),
    ("GG", Continent::Europe, "Guernsey"),
    ("GH", Continent::Africa, "Ghana"),
    ("GI", Continent::Europe, "Gibraltar"),
    ("GL", Continent::NorthAmerica, "Greenland"),
    ("GM", Continent::Africa, "Gambia"),
    ("GN", Continent::Africa, "Guinea"),
    ("GP", Continent::NorthAmerica, "Guadeloupe"),
    ("GQ", Continent::Africa, "Equatorial Guinea"),
    ("GR", Continent::Europe, "Greece"),
    (
        "GS",
        Continent::Antarctica,
        "South Georgia and the South Sandwich Islands",
    ),
    ("GT", Continent::NorthAmerica, "Guatemala"),
    ("GU", Continent::Oceania, "Guam"),
    ("GW", Continent::Africa, "Guinea-Bissau"),
    ("GY", Continent::SouthAmerica, "Guyana"),
    ("HK", Continent::Asia, "Hong Kong"),
    (
        "HM",
        Continent::Antarctica,
        "Heard Island and McDonald Islands",
    ),
    ("HN", Continent::NorthAmerica, "Honduras"),
    ("HR", Continent::Europe, "Croatia"),
    ("HT", Continent::NorthAmerica, "Haiti"),
    ("HU", Continent::Europe, "Hungary"),
    ("ID", Continent::Asia, "Indonesia"),
    ("IE", Continent::Europe, "Ireland"),
    ("IL", Continent::Asia, "Israel"),
    ("IM", Continent::Europe, "Isle of Man"),
    ("IN", Continent::Asia, "India"),
    ("IO", Continent::Asia, "British Indian Ocean Territory"),
    ("IQ", Continent::Asia, "Iraq"),
    ("IR", Continent::Asia, "Iran"),
    ("IS", Continent::Europe, "Iceland"),
    ("IT", Continent::Europe, "Italy"),
    ("JE", Continent::Europe, "Jersey"),
    ("JM", Continent::NorthAmerica, "Jamaica"),
    ("JO", Continent::Asia, "Jordan"),
    ("JP", Continent::Asia, "Japan"),
    ("KE", Continent::Africa, "Kenya"),
    ("KG", Continent::Asia, "Kyrgyzstan"),
    ("KH", Continent::Asia, "Cambodia"),
    ("KI", Continent::Oceania, "Kiribati"),
    ("KM", Continent::Africa, "Comoros"),
    ("KN", Continent::NorthAmerica, "Saint Kitts and Nevis"),
    ("KP", Continent::Asia, "North Korea"),
    ("KR", Continent::Asia, "South Korea"),
    ("KW", Continent::Asia, "Kuwait"),
    ("KY", Continent::NorthAmerica, "Cayman Islands"),
    ("KZ", Continent::Asia, "Kazakhstan"),
    ("LA", Continent::Asia, "Laos"),
    ("LB", Continent::Asia, "Lebanon"),
    ("LC", Continent::NorthAmerica, "Saint Lucia"),
    ("LI", Continent::Europe, "Liechtenstein"),
    ("LK", Continent::Asia, "Sri Lanka"),
    ("LR", Continent::Africa, "Liberia"),
    ("LS", Continent::Africa, "Lesotho"),
    ("LT", Continent::Europe, "Lithuania"),
    ("LU", Continent::Europe, "Luxembourg"),
    ("LV", Continent::Europe, "Latvia"),
    ("LY", Continent::Africa, "Libya"),
    ("MA", Continent::Africa, "Morocco"),
    ("MC", Continent::Europe, "Monaco"),
    ("MD", Continent::Europe, "Moldova"),
    ("ME", Continent::Europe, "Montenegro"),
    ("MF", Continent::NorthAmerica, "Saint Martin"),
    ("MG", Continent::Africa, "Madagascar"),
    ("MH", Continent::Oceania, "Marshall Islands"),
    ("MK", Continent::Europe, "North Macedonia"),
    ("ML", Continent::Africa, "Mali"),
    ("MM", Continent::Asia, "Myanmar"),
    ("MN", Continent::Asia, "Mongolia"),
    ("MO", Continent::Asia, "Macao"),
    ("MP", Continent::Oceania, "Northern Mariana Islands"),
    ("MQ", Continent::NorthAmerica, "Martinique"),
    ("MR", Continent::Africa, "Mauritania"),
    ("MS", Continent::NorthAmerica, "Montserrat"),
    ("MT", Continent::Europe, "Malta"),
    ("MU", Continent::Africa, "Mauritius"),
    ("MV", Continent::Asia, "Maldives"),
    ("MW", Continent::Africa, "Malawi"),
    ("MX", Continent::NorthAmerica, "Mexico"),
    ("MY", Continent::Asia, "Malaysia"),
    ("MZ", Continent::Africa, "Mozambique"),
    ("NA", Continent::Africa, "Namibia"),
    ("NC", Continent::Oceania, "New Caledonia"),
    ("NE", Continent::Africa, "Niger"),
    ("NF", Continent::Oceania, "Norfolk Island"),
    ("NG", Continent::Africa, "Nigeria"),
    ("NI", Continent::NorthAmerica, "Nicaragua"),
    ("NL", Continent::Europe, "Netherlands"),
    ("NO", Continent::Europe, "Norway"),
    ("NP", Continent::Asia, "Nepal"),
    ("NR", Continent::Oceania, "Nauru"),
    ("NU", Continent::Oceania, "Niue"),
    ("NZ", Continent::Oceania, "New Zealand"),
    ("OM", Continent::Asia, "Oman"),
    ("PA", Continent::NorthAmerica, "Panama"),
    ("PE", Continent::SouthAmerica, "Peru"),
    ("PF", Continent::Oceania, "French Polynesia"),
    ("PG", Continent::Oceania, "Papua New Guinea"),
    ("PH", Continent::Asia, "Philippines"),
    ("PK", Continent::Asia, "Pakistan"),
    ("PL", Continent::Europe, "Poland"),
    ("PM", Continent::NorthAmerica, "Saint Pierre and Miquelon"),
    ("PN", Continent::Oceania, "Pitcairn Islands"),
    ("PR", Continent::NorthAmerica, "Puerto Rico"),
    ("PS", Continent::Asia, "Palestine"),
    ("PT", Continent::Europe, "Portugal"),
    ("PW", Continent::Oceania, "Palau"),
    ("PY", Continent::SouthAmerica, "Paraguay"),
    ("QA", Continent::Asia, "Qatar"),
    ("RE", Continent::Africa, "Réunion"),
    ("RO", Continent::Europe, "Romania"),
    ("RS", Continent::Europe, "Serbia"),
    ("RU", Continent::Europe, "Russia"),
    ("RW", Continent::Africa, "Rwanda"),
    ("SA", Continent::Asia, "Saudi Arabia"),
    ("SB", Continent::Oceania, "Solomon Islands"),
    ("SC", Continent::Africa, "Seychelles"),
    ("SD", Continent::Africa, "Sudan"),
    ("SE", Continent::Europe, "Sweden"),
    ("SG", Continent::Asia, "Singapore"),
    ("SH", Continent::Africa, "Saint Helena"),
    ("SI", Continent::Europe, "Slovenia"),
    ("SJ", Continent::Europe, "Svalbard and Jan Mayen"),
    ("SK", Continent::Europe, "Slovakia"),
    ("SL", Continent::Africa, "Sierra Leone"),
    ("SM", Continent::Europe, "San Marino"),
    ("SN", Continent::Africa, "Senegal"),
    ("SO", Continent::Africa, "Somalia"),
    ("SR", Continent::SouthAmerica, "Suriname"),
    ("SS", Continent::Africa, "South Sudan"),
    ("ST", Continent::Africa, "São Tomé and Príncipe"),
    ("SV", Continent::NorthAmerica, "El Salvador"),
    ("SX", Continent::NorthAmerica, "Sint Maarten"),
    ("SY", Continent::Asia, "Syria"),
    ("SZ", Continent::Africa, "Eswatini"),
    ("TC", Continent::NorthAmerica, "Turks and Caicos Islands"),
    ("TD", Continent::Africa, "Chad"),
    ("TF", Continent::Antarctica, "French Southern Territories"),
    ("TG", Continent::Africa, "Togo"),
    ("TH", Continent::Asia, "Thailand"),
    ("TJ", Continent::Asia, "Tajikistan"),
    ("TK", Continent::Oceania, "Tokelau"),
    ("TL", Continent::Oceania, "Timor-Leste"),
    ("TM", Continent::Asia, "Turkmenistan"),
    ("TN", Continent::Africa, "Tunisia"),
    ("TO", Continent::Oceania, "Tonga"),
    ("TR", Continent::Asia, "Türkiye"),
    ("TT", Continent::NorthAmerica, "Trinidad and Tobago"),
    ("TV", Continent::Oceania, "Tuvalu"),
    ("TW", Continent::Asia, "Taiwan"),
    ("TZ", Continent::Africa, "Tanzania"),
    ("UA", Continent::Europe, "Ukraine"),
    ("UG", Continent::Africa, "Uganda"),
    ("UM", Continent::Oceania, "U.S. Minor Outlying Islands"),
    ("US", Continent::NorthAmerica, "United States"),
    ("UY", Continent::SouthAmerica, "Uruguay"),
    ("UZ", Continent::Asia, "Uzbekistan"),
    ("VA", Continent::Europe, "Vatican City"),
    (
        "VC",
        Continent::NorthAmerica,
        "Saint Vincent and the Grenadines",
    ),
    ("VE", Continent::SouthAmerica, "Venezuela"),
    ("VG", Continent::NorthAmerica, "British Virgin Islands"),
    ("VI", Continent::NorthAmerica, "U.S. Virgin Islands"),
    ("VN", Continent::Asia, "Vietnam"),
    ("VU", Continent::Oceania, "Vanuatu"),
    ("WF", Continent::Oceania, "Wallis and Futuna"),
    ("WS", Continent::Oceania, "Samoa"),
    ("XK", Continent::Europe, "Kosovo"),
    ("YE", Continent::Asia, "Yemen"),
    ("YT", Continent::Africa, "Mayotte"),
    ("ZA", Continent::Africa, "South Africa"),
    ("ZM", Continent::Africa, "Zambia"),
    ("ZW", Continent::Africa, "Zimbabwe"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_iso_countries_and_kosovo() {
        let code = |value| Country::parse(value).map(Country::as_str);
        assert_eq!(code("fr"), Some("FR"));
        assert_eq!(code(" FR "), Some("FR"));
        assert_eq!(code("UK"), Some("GB"));
        assert_eq!(code("XK"), Some("XK"));
        for value in [
            "AB", "AN", "AP", "CS", "EU", "FX", "NT", "NY", "OS", "SU", "XG", "XW", "YU", "ZZ", "",
            "F", "FRA",
        ] {
            assert_eq!(code(value), None, "{value}");
        }
    }

    #[test]
    fn indexes_a_sorted_table() {
        assert!(COUNTRIES.windows(2).all(|w| w[0].0 < w[1].0));
        assert!(COUNTRIES.len() <= usize::from(u8::MAX) + 1);
        assert_eq!(Country::parse("ZW").unwrap().as_str(), "ZW");
        assert_eq!(Country::parse("AD").unwrap().continent(), Continent::Europe);
        assert!(
            EUROPEAN_UNION
                .iter()
                .all(|code| Country::parse(code).is_some())
        );
    }

    #[test]
    fn names_countries_like_geolite2() {
        let name = |code| Country::parse(code).unwrap().name();
        assert_eq!(name("FR"), "France");
        assert_eq!(name("UK"), "United Kingdom");
        assert_eq!(name("RU"), "Russia");
        assert_eq!(name("KR"), "South Korea");
        assert_eq!(name("CD"), "DR Congo");
        assert_eq!(name("XK"), "Kosovo");
        assert!(COUNTRIES.iter().all(|(_, _, name)| !name.is_empty()));
    }

    #[test]
    fn knows_the_european_union() {
        let member = |code| Country::parse(code).unwrap().is_in_european_union();
        assert!(member("FR") && member("CY") && member("HR"));
        assert!(!member("GB") && !member("CH") && !member("NO"));
    }
}
