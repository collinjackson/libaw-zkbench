//! A fast, non-cryptographic hasher for the engine's and bot's hot maps (rustc's Fx hash). The
//! std default (SipHash, built to resist hash flooding) was 28% of a self-play server's work
//! (profile 2026-11-12); none of these maps takes untrusted keys.

use core::hash::{BuildHasherDefault, Hasher};
use hashbrown::{HashMap, HashSet};

pub type FxMap<K, V> = HashMap<K, V, BuildHasherDefault<FxHasher>>;
pub type FxSet<K> = HashSet<K, BuildHasherDefault<FxHasher>>;

const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

#[derive(Default, Clone, Copy)]
pub struct FxHasher {
    hash: u64,
}

impl FxHasher {
    #[inline]
    fn add(&mut self, w: u64) {
        self.hash = (self.hash.rotate_left(5) ^ w).wrapping_mul(SEED);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut c = bytes.chunks_exact(8);
        for w in &mut c {
            self.add(u64::from_le_bytes(w.try_into().unwrap()));
        }
        let r = c.remainder();
        if !r.is_empty() {
            let mut b = [0u8; 8];
            b[..r.len()].copy_from_slice(r);
            self.add(u64::from_le_bytes(b));
        }
    }
    #[inline]
    fn write_u8(&mut self, i: u8) { self.add(i as u64) }
    #[inline]
    fn write_u16(&mut self, i: u16) { self.add(i as u64) }
    #[inline]
    fn write_u32(&mut self, i: u32) { self.add(i as u64) }
    #[inline]
    fn write_u64(&mut self, i: u64) { self.add(i) }
    #[inline]
    fn write_usize(&mut self, i: usize) { self.add(i as u64) }
    #[inline]
    fn finish(&self) -> u64 { self.hash }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;
    #[test]
    fn maps_and_sets_work() {
        let mut m: FxMap<(u16, u16), u32> = FxMap::default();
        for x in 0..40u16 { for y in 0..40u16 { m.insert((x, y), (x * 100 + y) as u32); } }
        assert_eq!(m.len(), 1600);
        assert_eq!(m[&(13, 7)], 1307);
        let s: FxSet<String> = ["Infantry", "Mech", "Tank"].iter().map(|s| s.to_string()).collect();
        assert!(s.contains("Mech") && !s.contains("Neotank"));
    }
}
