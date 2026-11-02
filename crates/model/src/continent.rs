use crate::country::Country;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Continent {
    Africa,
    Antarctica,
    Asia,
    Europe,
    NorthAmerica,
    Oceania,
    SouthAmerica,
}

impl Continent {
    pub const ALL: [Continent; 7] = [
        Continent::Africa,
        Continent::Antarctica,
        Continent::Asia,
        Continent::Europe,
        Continent::NorthAmerica,
        Continent::Oceania,
        Continent::SouthAmerica,
    ];

    pub fn code(self) -> &'static str {
        match self {
            Continent::Africa => "AF",
            Continent::Antarctica => "AN",
            Continent::Asia => "AS",
            Continent::Europe => "EU",
            Continent::NorthAmerica => "NA",
            Continent::Oceania => "OC",
            Continent::SouthAmerica => "SA",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Continent::Africa => "Africa",
            Continent::Antarctica => "Antarctica",
            Continent::Asia => "Asia",
            Continent::Europe => "Europe",
            Continent::NorthAmerica => "North America",
            Continent::Oceania => "Oceania",
            Continent::SouthAmerica => "South America",
        }
    }
}

pub fn continent(country: &str) -> Option<Continent> {
    Country::parse(country).map(Country::continent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_countries_to_continents() {
        assert_eq!(continent("FR"), Some(Continent::Europe));
        assert_eq!(continent("NA"), Some(Continent::Africa));
        assert_eq!(continent("BR"), Some(Continent::SouthAmerica));
        assert_eq!(continent("XK"), Some(Continent::Europe));
        assert_eq!(continent("XX"), None);
    }
}
