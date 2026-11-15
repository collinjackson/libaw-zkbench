//! The referee program for proving games in a zkVM. It takes a position, a list of actions
//! (in the HTTP contract's JSON form) and a luck seed, checks that each action is legal,
//! applies it, and reports the winner, the day and a hash of the final position.
//!
//! It is plain Rust with no zkVM-specific calls, so every zkVM runs the same code and the
//! benchmark (README.md) compares like with like. Luck comes from a small
//! deterministic generator keyed by the seed; hashing is FNV-1a, so no zkVM's built-in hash
//! is used.
#![no_std]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use libaw::actions::is_legal;
use libaw::sim::step;
use libaw::state::State;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize)]
pub struct Input {
    pub state: State,
    pub actions: Vec<Value>,
    pub seed: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Output {
    pub actions: u32,
    pub day: u32,
    pub winner: Option<String>,
    pub state_hash: u64,
}

/// The luck stream: splitmix64, one draw per roll, scaled into the roll's range.
pub struct Rolls(u64);

impl Rolls {
    pub fn new(seed: u64) -> Self {
        Rolls(seed)
    }
    pub fn roll(&mut self, lo: i32, hi: i32) -> i32 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        lo + (z % (hi - lo + 1) as u64) as i32
    }
}

pub fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3))
}

/// Check and apply every action. An illegal or unparseable action is an error, naming its index.
pub fn run(input: Input) -> Result<Output, String> {
    let Input { mut state, actions, seed } = input;
    let mut rolls = Rolls::new(seed);
    for (i, v) in actions.iter().enumerate() {
        let a = libaw::awbw::action_from_contract(&state, v).map_err(|e| format!("action {i}: {e}"))?;
        if !is_legal(&state, &a) {
            return Err(format!("action {i}: not legal"));
        }
        step(&mut state, &a, &mut |lo, hi| rolls.roll(lo, hi)).map_err(|e| format!("action {i}: {e}"))?;
    }
    Ok(Output {
        actions: actions.len() as u32,
        day: state.day as u32,
        winner: libaw::sim::winner(&state),
        state_hash: fnv1a(&serde_json::to_vec(&state).unwrap()),
    })
}
