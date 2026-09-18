use std::vec;

use blake3;

pub struct FeistelBytes(Vec<u8>);

impl FeistelBytes {
    pub fn new(data: Vec<u8>) -> Result<FeistelBytes, &'static str> {
        if data.len() % 2 == 0 {
            Ok(FeistelBytes(data))
        } else {
            Err("FeistelBytes must be even.")
        }
    }

    fn halves(&self) -> (&[u8], &[u8]) {
        self.0.split_at(self.0.len() / 2)
    }

    pub fn forward(&mut self, f_out: &[u8]) {
        let mid = self.0.len() / 2;
        let (right, left) = self.0.split_at_mut(mid);

        assert_eq!(f_out.len(), right.len(), "f_out must be half len");

        for i in 0..left.len() {
            let old_left = left[i];
            left[i] = right[i];
            right[i] = old_left ^ f_out[i];
        }
    }

    pub fn backward(&mut self, f_out: &[u8]) {
        let mid = self.0.len() / 2;
        let (right, left) = self.0.split_at_mut(mid);

        assert_eq!(f_out.len(), right.len(), "f_out must match half len");

        for i in 0..right.len() {
            let old_left = left[i];
            left[i] = right[i] ^ f_out[i];
            right[i] = old_left;
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

pub struct BabelMachine {
    pub key: [u8; 32],
    pub rounds: u8,
}

impl BabelMachine {
    pub fn new(key: [u8; 32], rounds: u8) -> Self {
        BabelMachine { key, rounds }
    }

    fn feistel_step(&self, round: u8, right: &[u8], out: &mut [u8]) {
        let mut hasher = blake3::Hasher::new_keyed(&self.key);
        hasher.update(&[round]);
        hasher.update(right);
        hasher.finalize_xof().fill(out);
    }

    pub fn forward(&self, input: &mut FeistelBytes) {
        let half_len = input.0.len() / 2;
        let mut f_out = vec![0u8; half_len];
        for r in 0..self.rounds {
            let (right, _left) = input.halves();
            self.feistel_step(r, right, &mut f_out);
            input.forward(&f_out);
        }
    }

    pub fn backward(&self, input: &mut FeistelBytes) {
        let half_len = input.0.len() / 2;
        let mut f_out = vec![0u8; half_len];

        for r in (0..self.rounds).rev() {
            let (_right, left) = input.halves();
            self.feistel_step(r, left, &mut f_out); // hash the OTHER half this time
            input.backward(&f_out);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn even_len_accept() {
        let data = vec![1, 2, 3, 4];
        assert!(FeistelBytes::new(data).is_ok())
    }

    #[test]
    fn odd_len_rejected() {
        let data = vec![1, 2, 3];
        assert!(FeistelBytes::new(data).is_err());
    }

    #[test]
    fn forward_then_backward_recovers_original() {
        let key = [7u8; 32];
        let machine = BabelMachine::new(key, 4);
        let original = vec![1, 2, 3, 4, 5, 6, 7, 8];

        let mut block = FeistelBytes::new(original.clone()).unwrap();
        machine.forward(&mut block);
        assert_ne!(block.0, original); // sanity: it actually changed

        machine.backward(&mut block);
        assert_eq!(block.0, original); // and now it's back
    }

    #[test]
    fn feistel_round_is_bijective_2_byte_block() {
        let key = [7u8; 32];
        let machine = BabelMachine::new(key, 4);

        let n: u32 = 1 << 16; // 65,536
        let mut seen = vec![false; n as usize]; // 65,536 bools = 64KB

        for i in 0..n {
            let bytes = (i as u16).to_be_bytes().to_vec();
            let mut block = FeistelBytes::new(bytes).unwrap();
            machine.forward(&mut block);

            let out_bytes = block.as_bytes();
            let out_index = u16::from_be_bytes([out_bytes[0], out_bytes[1]]) as usize;

            assert!(
                !seen[out_index],
                "collision detected at output {}",
                out_index
            );
            seen[out_index] = true;
        }

        // every output was hit exactly once -> function is a bijection on this domain
        assert!(seen.iter().all(|&b| b));
    }
}
