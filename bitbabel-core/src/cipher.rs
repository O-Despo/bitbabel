//! All of the actual computation lives here.
//!
//! A Feistel network turns any keyed round function into an invertible permutation of a
//! fixed-length block. [`BabelMachine`] runs a set of Feistel rounds forward, and the same
//! rounds in reverse to undo them. [`FeistelBytes`] is the block being permuted; it does the
//! splitting and swapping of halves so the machine never handles raw slicing.

use crate::error::CipherError;

/// An even-length byte block that can apply one Feistel round, or its inverse, to its halves.
///
/// The round methods are private and take the round function as a closure rather than its
/// output: the block allocates a buffer of exactly half its length and hands it to the
/// closure, so a round-function output of the wrong length cannot be constructed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeistelBytes(Vec<u8>);

impl FeistelBytes {
    /// Wraps `data` as a block.
    ///
    /// # Errors
    ///
    /// Returns [`CipherError::OddBlockLen`] if `data` cannot be split into equal halves.
    pub fn new(data: Vec<u8>) -> Result<Self, CipherError> {
        if data.len() % 2 == 0 {
            Ok(FeistelBytes(data))
        } else {
            Err(CipherError::OddBlockLen(data.len()))
        }
    }

    /// One Feistel round: `(first, second) -> (second ^ F(first), first)`.
    ///
    /// `f` is the round function F. It reads `first` and fills the half-length `out`.
    fn forward_round(&mut self, f: impl FnOnce(&[u8], &mut [u8])) {
        let mid = self.0.len() / 2;
        let (first, second) = self.0.split_at_mut(mid);

        // `first`, `second` and `f_out` all have length `mid`, so the zip below never truncates.
        let mut f_out = vec![0u8; mid];
        f(first, &mut f_out);

        for ((a, b), f) in first.iter_mut().zip(second.iter_mut()).zip(&f_out) {
            let old_first = *a;
            *a = *b ^ f;
            *b = old_first;
        }
    }

    /// Inverse of [`forward_round`](Self::forward_round): `(x, y) -> (y, x ^ F(y))`.
    ///
    /// `f` is the same round function F, but reads the current second half `y`, which is the
    /// `first` that the forward round started from.
    fn backward_round(&mut self, f: impl FnOnce(&[u8], &mut [u8])) {
        let mid = self.0.len() / 2;
        let (first, second) = self.0.split_at_mut(mid);

        let mut f_out = vec![0u8; mid];
        f(second, &mut f_out);

        for ((a, b), f) in first.iter_mut().zip(second.iter_mut()).zip(&f_out) {
            let old_first = *a;
            *a = *b;
            *b = old_first ^ f;
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

/// A keyed Feistel permutation over [`FeistelBytes`].
///
/// [`forward`](Self::forward) runs `rounds` Feistel steps; [`backward`](Self::backward)
/// runs the same steps in reverse order and recovers the original block exactly.
#[derive(Clone)]
pub struct BabelMachine {
    key: [u8; 32],
    rounds: u8,
}

// Hand-written so the key never ends up in logs or panic messages.
impl std::fmt::Debug for BabelMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BabelMachine")
            .field("key", &"<redacted>")
            .field("rounds", &self.rounds)
            .finish()
    }
}

impl BabelMachine {
    /// # Errors
    ///
    /// Returns [`CipherError::ZeroRounds`] if `rounds` is 0, which would be the identity.
    pub fn new(key: [u8; 32], rounds: u8) -> Result<Self, CipherError> {
        if rounds == 0 {
            Err(CipherError::ZeroRounds)
        } else {
            Ok(BabelMachine { key, rounds })
        }
    }

    pub fn rounds(&self) -> u8 {
        self.rounds
    }

    /// The Feistel round function F: keyed BLAKE3 of `[round] || half`, expanded to fill `out`.
    fn feistel_step(&self, round: u8, half: &[u8], out: &mut [u8]) {
        let mut hasher = blake3::Hasher::new_keyed(&self.key);
        hasher.update(&[round]);
        hasher.update(half);
        hasher.finalize_xof().fill(out);
    }

    /// Runs rounds `0..rounds` in order.
    pub fn forward(&self, input: &mut FeistelBytes) {
        for round in 0..self.rounds {
            input.forward_round(|half, out| self.feistel_step(round, half, out));
        }
    }

    /// Runs rounds `rounds-1..=0` in reverse, undoing [`forward`](Self::forward).
    pub fn backward(&self, input: &mut FeistelBytes) {
        for round in (0..self.rounds).rev() {
            input.backward_round(|half, out| self.feistel_step(round, half, out));
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn machine() -> BabelMachine {
        BabelMachine::new([7u8; 32], 4).unwrap()
    }

    #[test]
    fn even_len_accept() {
        assert!(FeistelBytes::new(vec![1, 2, 3, 4]).is_ok());
    }

    #[test]
    fn odd_len_rejected() {
        assert_eq!(
            FeistelBytes::new(vec![1, 2, 3]),
            Err(CipherError::OddBlockLen(3))
        );
    }

    #[test]
    fn zero_rounds_rejected() {
        assert_eq!(
            BabelMachine::new([0u8; 32], 0).unwrap_err(),
            CipherError::ZeroRounds
        );
    }

    #[test]
    fn debug_redacts_key() {
        let shown = format!("{:?}", BabelMachine::new([7u8; 32], 4).unwrap());
        assert!(shown.contains("redacted"));
        assert!(!shown.contains('7'));
    }

    #[test]
    fn forward_then_backward_recovers_original() {
        let original = vec![1, 2, 3, 4, 5, 6, 7, 8];

        let mut block = FeistelBytes::new(original.clone()).unwrap();
        machine().forward(&mut block);
        assert_ne!(block.as_bytes(), original); // sanity: it actually changed

        machine().backward(&mut block);
        assert_eq!(block.as_bytes(), original); // and now it's back
    }

    #[test]
    fn empty_block_is_a_no_op() {
        let mut block = FeistelBytes::new(vec![]).unwrap();
        machine().forward(&mut block);
        machine().backward(&mut block);
        assert!(block.as_bytes().is_empty());
    }

    #[test]
    fn feistel_round_is_bijective_2_byte_block() {
        let machine = machine();

        let n: u32 = 1 << 16; // 65,536
        let mut seen = vec![false; n as usize]; // 65,536 bools = 64KB

        for i in 0..n {
            let bytes = (i as u16).to_be_bytes().to_vec();
            let mut block = FeistelBytes::new(bytes).unwrap();
            machine.forward(&mut block);

            let out_bytes = block.as_bytes();
            let out_index = u16::from_be_bytes([out_bytes[0], out_bytes[1]]) as usize;

            assert!(!seen[out_index], "collision detected at output {out_index}");
            seen[out_index] = true;
        }

        // every output was hit exactly once -> function is a bijection on this domain
        assert!(seen.iter().all(|&b| b));
    }
}
