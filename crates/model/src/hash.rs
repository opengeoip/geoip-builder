const OFFSET: u64 = 0xcbf29ce484222325;
const PRIME: u64 = 0x100000001b3;

pub fn fnv1a(seed: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(OFFSET ^ seed, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(PRIME)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_reference_fnv1a() {
        assert_eq!(fnv1a(0, b""), OFFSET);
        assert_eq!(fnv1a(0, b"a"), 0xaf63dc4c8601ec8c);
        assert_eq!(fnv1a(0, b"foobar"), 0x85944171f73967e8);
        assert_ne!(fnv1a(1, b"foobar"), fnv1a(0, b"foobar"));
    }
}
