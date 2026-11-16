//! Plays one bot turn natively (wasmi on the host), for checking the zkVM run.
//!   cargo run --release -p libaw-turn --bin turn-native -- zkvm/workloads/turn.json BOT.wasm
use libaw_turn::{play_turn, Input};
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let w: libaw_proof::Input = serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let wasm = std::fs::read(&args[2]).unwrap();
    let input = Input { state: w.state, seed: w.seed, max_actions: 400, fuel_per_action: 2_000_000_000 };
    let t = Instant::now();
    let (out, _) = play_turn(input, &wasm, || 0).unwrap();
    println!("{} actions in {:.3}s", out.moves.len(), t.elapsed().as_secs_f64());
    println!("{}", serde_json::to_string(&out).unwrap());
}
