//! CPython's `random` module, reimplemented for the calls this program makes.
//!
//! The layout of every render is a Mersenne Twister stream keyed by a *string*
//! seed, and the golden corpus pins that stream byte for byte. Choosing a Rust
//! RNG would have been less code and would have moved every hexagon, so this
//! reproduces CPython exactly: MT19937 with `init_by_array`, seeded the way
//! `random.seed(str)` seeds, and the same rejection-sampling and pool
//! algorithms on top -- `_randbelow`, `shuffle` and `sample` all consume a
//! specific number of 32-bit words, so a "compatible" shortcut desynchronises
//! everything drawn after it.
use sha2::{Digest, Sha512};

const N: usize = 624;
const M: usize = 397;
const MATRIX_A: u32 = 0x9908_b0df;
const UPPER_MASK: u32 = 0x8000_0000;
const LOWER_MASK: u32 = 0x7fff_ffff;

/// One `random.Random` instance.
pub struct PyRandom {
    mt: [u32; N],
    mti: usize,
}

impl PyRandom {
    /// `random.Random(s)` for a string seed. CPython turns the string into an
    /// integer -- `int.from_bytes(s + sha512(s), "big")` -- then splits that
    /// integer into little-endian 32-bit words and calls `init_by_array`. The
    /// sha512 is not decoration: it is what makes short, similar seeds like
    /// "star:0:3:4" and "star:0:3:5" produce unrelated streams.
    pub fn new(seed: &str) -> Self {
        let mut be = seed.as_bytes().to_vec();
        be.extend_from_slice(&Sha512::digest(seed.as_bytes()));

        // The integer's magnitude, so leading zero bytes carry no words.
        let lead = be.iter().take_while(|b| **b == 0).count();
        let be = &be[lead..];
        let key: Vec<u32> = if be.is_empty() {
            vec![0]
        } else {
            let bits = be.len() * 8 - be[0].leading_zeros() as usize;
            let words = (bits - 1) / 32 + 1;
            let mut le = be.to_vec();
            le.reverse();
            le.resize(words * 4, 0);
            le.chunks(4)
                .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
                .collect()
        };
        Self::init_by_array(&key)
    }

    fn init_genrand(s: u32) -> Self {
        let mut mt = [0u32; N];
        mt[0] = s;
        for i in 1..N {
            mt[i] = 1812433253u32
                .wrapping_mul(mt[i - 1] ^ (mt[i - 1] >> 30))
                .wrapping_add(i as u32);
        }
        Self { mt, mti: N }
    }

    fn init_by_array(key: &[u32]) -> Self {
        let mut r = Self::init_genrand(19650218);
        let (mut i, mut j) = (1usize, 0usize);
        for _ in 0..N.max(key.len()) {
            r.mt[i] = (r.mt[i] ^ ((r.mt[i - 1] ^ (r.mt[i - 1] >> 30)).wrapping_mul(1664525)))
                .wrapping_add(key[j])
                .wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= N {
                r.mt[0] = r.mt[N - 1];
                i = 1;
            }
            if j >= key.len() {
                j = 0;
            }
        }
        for _ in 0..N - 1 {
            r.mt[i] = (r.mt[i] ^ ((r.mt[i - 1] ^ (r.mt[i - 1] >> 30)).wrapping_mul(1566083941)))
                .wrapping_sub(i as u32);
            i += 1;
            if i >= N {
                r.mt[0] = r.mt[N - 1];
                i = 1;
            }
        }
        r.mt[0] = 0x8000_0000;
        r
    }

    fn genrand_u32(&mut self) -> u32 {
        if self.mti >= N {
            for i in 0..N {
                let y = (self.mt[i] & UPPER_MASK) | (self.mt[(i + 1) % N] & LOWER_MASK);
                self.mt[i] =
                    self.mt[(i + M) % N] ^ (y >> 1) ^ if y & 1 != 0 { MATRIX_A } else { 0 };
            }
            self.mti = 0;
        }
        let mut y = self.mt[self.mti];
        self.mti += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^ (y >> 18)
    }

    /// `random()` -- 53 bits of randomness, assembled from two words exactly as
    /// `genrand_res53` does. Two words, always: taking one would halve the
    /// stream consumption and desynchronise everything after it.
    pub fn random(&mut self) -> f64 {
        let a = self.genrand_u32() >> 5;
        let b = self.genrand_u32() >> 6;
        (a as f64 * 67108864.0 + b as f64) * (1.0 / 9007199254740992.0)
    }

    /// `getrandbits(k)` for `1 <= k <= 32`, which is all this program needs.
    pub fn getrandbits(&mut self, k: u32) -> u32 {
        assert!((1..=32).contains(&k), "getrandbits({k}) is out of range");
        self.genrand_u32() >> (32 - k)
    }

    /// `_randbelow(n)` -- rejection sampling on `n.bit_length()`, so the number
    /// of words consumed depends on the values drawn. A modulo shortcut would
    /// be both biased and desynchronising.
    fn randbelow(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        let k = 32 - n.leading_zeros();
        loop {
            let r = self.getrandbits(k);
            if r < n {
                return r;
            }
        }
    }

    /// `randrange(n)`.
    pub fn randrange(&mut self, n: u32) -> u32 {
        self.randbelow(n)
    }

    /// `shuffle(x)` -- Fisher-Yates walking down, matching CPython's loop
    /// direction.
    pub fn shuffle<T>(&mut self, xs: &mut [T]) {
        for i in (1..xs.len()).rev() {
            let j = self.randbelow(i as u32 + 1) as usize;
            xs.swap(i, j);
        }
    }

    /// `sample(range(n), k)`. CPython picks between a pool and a set algorithm
    /// by size; every call here is `sample(range(6), 6)`, far below the
    /// threshold (`setsize` = 85 for k=6), so the pool branch is the only one
    /// that can be taken.
    pub fn sample(&mut self, n: usize, k: usize) -> Vec<usize> {
        let mut pool: Vec<usize> = (0..n).collect();
        let mut out = Vec::with_capacity(k);
        for i in 0..k {
            let j = self.randbelow((n - i) as u32) as usize;
            out.push(pool[j]);
            pool[j] = pool[n - i - 1];
        }
        out
    }

    /// `uniform(a, b)`.
    pub fn uniform(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.random()
    }

    /// `choice(seq)` over an ASCII set.
    pub fn choice(&mut self, s: &[u8]) -> u8 {
        s[self.randbelow(s.len() as u32) as usize]
    }
}

/// Per-cell stream. Keyed by coordinates rather than draw order so a cell's
/// stars never shift because some *other* cell consumed a different number of
/// values -- that is what keeps the layout identical across every background,
/// icon and overlay combination.
pub fn cell_rng(tag: &str, seed: u32, r: i32, c: i32) -> PyRandom {
    PyRandom::new(&format!("{tag}:{seed}:{r}:{c}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vectors captured from the interpreter this corpus was generated with
    /// (CPython 3.14.7). If any of these move, every hexagon in every golden
    /// moves with them.
    #[test]
    fn matches_cpython() {
        let mut r = PyRandom::new("trihex:0");
        assert_eq!(
            [r.random(), r.random(), r.random()],
            [0.323979587515701, 0.480793333456907, 0.521798912248572]
        );

        let mut r = PyRandom::new("trihex:0");
        assert_eq!(
            [
                r.getrandbits(32),
                r.getrandbits(32),
                r.getrandbits(32),
                r.getrandbits(32)
            ],
            [1391481750, 664579869, 2064991622, 3668470967]
        );

        // random.sample(range(6), 6) -- the call pat_trihex makes per hexagon
        let mut r = PyRandom::new("trihex:0");
        assert_eq!(r.sample(6, 6), vec![2, 1, 3, 5, 4, 0]);

        let mut r = PyRandom::new("trihex:0");
        let mut xs: Vec<u32> = (0..10).collect();
        r.shuffle(&mut xs);
        assert_eq!(xs, vec![9, 8, 1, 0, 3, 4, 6, 7, 2, 5]);

        // the per-column draws pat_matrix makes, in order
        assert_eq!(
            PyRandom::new("rain:0:0:3").uniform(18.0, 34.0),
            18.93273603129014
        );
        assert_eq!(PyRandom::new("rain:0:0:3").randrange(97), 7);
        assert_eq!(
            PyRandom::new("star:0:-1:-1").choice(b"0123456789ABCDEF"),
            b'2'
        );
    }
}
