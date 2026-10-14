//! `com.company.util.Random`: the Park–Miller generator (seed × 16807 mod 2³¹−1), seeded with
//! the map's seed on MapInfo. The server runs the same generator with the same seed, so every
//! roll the AIR client makes (shot damage, ground damage) must happen here in the same order.

#[derive(Debug, Clone)]
pub struct Random {
    seed: u32,
}

impl Random {
    pub fn new(seed: u32) -> Self {
        Self { seed }
    }

    /// `gen()`, in AS3's uint arithmetic (the sums stay below 2³²).
    fn gen(&mut self) -> u32 {
        let mut lo: u64 = 16807 * (self.seed & 0xFFFF) as u64;
        let hi: u64 = 16807 * (self.seed >> 16) as u64;
        lo += (hi & 0x7FFF) << 16;
        lo += hi >> 15;
        if lo > 0x7FFF_FFFF {
            lo -= 0x7FFF_FFFF;
        }
        self.seed = lo as u32;
        self.seed
    }

    pub fn next_int(&mut self) -> u32 {
        self.gen()
    }

    /// `nextIntRange(min, max)`: min ≤ n < max (or min when they're equal).
    pub fn next_int_range(&mut self, min: u32, max: u32) -> u32 {
        if min == max {
            min
        } else {
            min.wrapping_add(self.gen() % max.wrapping_sub(min))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minstd_sequence() {
        // The published MINSTD sequence from seed 1.
        let mut r = Random::new(1);
        assert_eq!(r.next_int(), 16807);
        assert_eq!(r.next_int(), 282475249);
        assert_eq!(r.next_int(), 1622650073);
        assert_eq!(r.next_int(), 984943658);
        // The 10000th value of MINSTD from seed 1 is 1043618065.
        let mut r = Random::new(1);
        let mut last = 0;
        for _ in 0..10000 {
            last = r.next_int();
        }
        assert_eq!(last, 1043618065);
    }

    #[test]
    fn int_range() {
        let mut r = Random::new(1);
        // Equal bounds don't use up a value.
        assert_eq!(r.next_int_range(10, 10), 10);
        assert_eq!(r.next_int_range(10, 20), 10 + 16807 % 10);
    }
}
