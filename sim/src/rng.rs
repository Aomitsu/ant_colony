use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};

/// PRNGEngine uses ChaCha8 generator to create deterministic multi plateform RNG
pub struct PRNGEngine {
    /// ChaCha8 deterministic generator
    rng: ChaCha8Rng,
}

impl PRNGEngine {
    /// Setup a new PRNG Engine based on a u64 seed
    pub fn new(seed: u64) -> Self {
        let rng = ChaCha8Rng::seed_from_u64(seed);
        Self { rng }
    }
    /// Generate next `u32` number
    pub fn next_u32(&mut self) -> u32 {
        self.rng.next_u32()
    }
    /// Generate next `u64` number
    pub fn next_u64(&mut self) -> u64 {
        self.rng.next_u64()
    }
    /// Get the offset from the start of the stream, in 32-bit words.
    pub fn word_pos(&self) -> u128 {
        self.rng.get_word_pos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prngengine_deterministic_test_sequence_u32() {
        let mut rng1 = PRNGEngine::new(42);
        let mut rng2 = PRNGEngine::new(42);

        let mut array1: Vec<u32> = vec![];
        let mut array2: Vec<u32> = vec![];

        for _ in 0..100 {
            array1.push(rng1.next_u32());
            array2.push(rng2.next_u32());
        }

        assert_eq!(array1, array2)
    }

    #[test]
    fn prngengine_deterministic_test_sequence_u64() {
        let mut rng1 = PRNGEngine::new(42);
        let mut rng2 = PRNGEngine::new(42);

        let mut array1: Vec<u64> = vec![];
        let mut array2: Vec<u64> = vec![];

        for _ in 0..100 {
            array1.push(rng1.next_u64());
            array2.push(rng2.next_u64());
        }

        assert_eq!(array1, array2)
    }

    #[test]
    fn prngengine_deterministic_different_seeds() {
        let mut rng1 = PRNGEngine::new(42);
        let mut rng2 = PRNGEngine::new(41);

        let mut array1: Vec<u64> = vec![];
        let mut array2: Vec<u64> = vec![];

        for _ in 0..100 {
            array1.push(rng1.next_u64());
            array2.push(rng2.next_u64());
        }

        assert_ne!(array1, array2)
    }

    #[test]
    fn prngengine_deterministic_golden_sequence_u32() {
        let mut rng = PRNGEngine::new(42);
        let expected: [u32; 5] = [962419617, 2928721845, 628724104, 4081401798, 3317060492];

        for e in expected {
            assert_eq!(rng.next_u32(), e)
        }
    }

    #[test]
    fn prngengine_deterministic_golden_sequence_u64() {
        let mut rng = PRNGEngine::new(42);
        let expected: [u64; 5] = [
            12578764544318200737,
            17529487244874322312,
            7886285670807131020,
            11572758976476374866,
            5323617429756461744,
        ];

        for e in expected {
            assert_eq!(rng.next_u64(), e)
        }
    }

    #[test]
    fn prngengine_word_pos_starts_at_zero() {
        let rng = PRNGEngine::new(42);

        assert_eq!(rng.word_pos(), 0)
    }

    #[test]
    fn prngengine_word_pos_counts_consumed_words() {
        let mut rng = PRNGEngine::new(42);

        rng.next_u32();
        assert_eq!(rng.word_pos(), 1);

        rng.next_u64();
        assert_eq!(rng.word_pos(), 3)
    }

    #[test]
    fn prngengine_word_pos_tracks_across_block_boundary() {
        let mut rng = PRNGEngine::new(42);

        for i in 0..100u128 {
            assert_eq!(rng.word_pos(), i);
            rng.next_u32();
        }

        assert_eq!(rng.word_pos(), 100)
    }

    #[test]
    fn prngengine_word_pos_stable_without_draw() {
        let rng = PRNGEngine::new(42);

        assert_eq!(rng.word_pos(), rng.word_pos())
    }
}
