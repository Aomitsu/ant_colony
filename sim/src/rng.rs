use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};

// Never renumber discriminants, it breaks determinism. Append-only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u64)]
pub enum Flow {
    // 0 is reserved
    Terrain = 1,
}

impl Flow {
    pub const ALL: &[Flow] = &[Flow::Terrain];
    pub fn stream_id(self) -> u64 {
        self as u64
    }
    fn index(self) -> usize {
        self as usize - 1
    }
}

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
    /// Setup an engine sharing `seed` but with an independent ChaCha stream
    pub fn with_stream(seed: u64, stream: u64) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        rng.set_stream(stream);
        Self { rng }
    }
    /// Setup the engine dedicated to a `Flow`, derived from the world seed
    pub fn for_flow(seed: u64, flow: Flow) -> Self {
        Self::with_stream(seed, flow.stream_id())
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

/// Owns one independent `PRNGEngine` per `Flow`, all derived from the world seed
pub struct Rngs {
    /// Indexed like `Flow::ALL` (deterministic order, no hashing)
    engines: Vec<PRNGEngine>,
}

impl Rngs {
    /// Build every `Flow` engine from a single world seed
    pub fn new(world_seed: u64) -> Self {
        let engines = Flow::ALL
            .iter()
            .map(|&flow| PRNGEngine::for_flow(world_seed, flow))
            .collect();
        Self { engines }
    }
    /// Borrow the engine dedicated to `flow`
    pub fn get(&mut self, flow: Flow) -> &mut PRNGEngine {
        &mut self.engines[flow.index()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn flow_stream_ids_are_unique_and_non_zero() {
        for (i, &flow) in Flow::ALL.iter().enumerate() {
            let id = flow.stream_id();
            assert_ne!(id, 0, "stream 0 is reserved for PRNGEngine::new");
            assert_eq!(id as usize, i + 1, "Flow ids must be contiguous from 1");
        }
    }

    #[test]
    fn with_stream_same_stream_is_deterministic() {
        let mut rng1 = PRNGEngine::with_stream(42, 7);
        let mut rng2 = PRNGEngine::with_stream(42, 7);

        for _ in 0..100 {
            assert_eq!(rng1.next_u64(), rng2.next_u64());
        }
    }

    #[test]
    fn with_stream_different_streams_differ() {
        let mut rng1 = PRNGEngine::with_stream(42, 1);
        let mut rng2 = PRNGEngine::with_stream(42, 2);

        let array1: Vec<u64> = (0..100).map(|_| rng1.next_u64()).collect();
        let array2: Vec<u64> = (0..100).map(|_| rng2.next_u64()).collect();

        assert_ne!(array1, array2);
    }

    #[test]
    fn for_flow_matches_with_stream() {
        let mut rng1 = PRNGEngine::for_flow(42, Flow::Terrain);
        let mut rng2 = PRNGEngine::with_stream(42, Flow::Terrain.stream_id());

        assert_eq!(rng1.next_u64(), rng2.next_u64());
    }

    #[test]
    fn rngs_new_is_deterministic() {
        let mut rngs1 = Rngs::new(42);
        let mut rngs2 = Rngs::new(42);

        for &flow in Flow::ALL {
            assert_eq!(rngs1.get(flow).next_u64(), rngs2.get(flow).next_u64());
        }
    }

    #[test]
    fn rngs_different_world_seeds_differ() {
        let mut rngs1 = Rngs::new(42);
        let mut rngs2 = Rngs::new(43);

        let array1: Vec<u64> = Flow::ALL.iter().map(|&f| rngs1.get(f).next_u64()).collect();
        let array2: Vec<u64> = Flow::ALL.iter().map(|&f| rngs2.get(f).next_u64()).collect();

        assert_ne!(array1, array2);
    }

    #[test]
    fn rngs_flows_are_independent() {
        let flow = Flow::Terrain;

        let mut rngs = Rngs::new(42);
        let _ = rngs.get(flow).next_u64();

        // Drawing from the other flows must not perturb `flow`.
        for &other in Flow::ALL.iter().filter(|&&f| f != flow) {
            let _ = rngs.get(other).next_u64();
        }
        let second = rngs.get(flow).next_u64();

        let mut solo = PRNGEngine::for_flow(42, flow);
        let _ = solo.next_u64();
        let expected_second = solo.next_u64();

        assert_eq!(second, expected_second);
    }

    #[test]
    fn for_flow_golden_sequence_u64() {
        let mut rng = PRNGEngine::for_flow(42, Flow::Terrain);
        let expected: [u64; 3] = [
            13222472167927179408,
            3078952320862533021,
            8898984633443201687,
        ];

        for e in expected {
            assert_eq!(rng.next_u64(), e)
        }
    }
}
