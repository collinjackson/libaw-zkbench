//! Plays a whole game between two bots natively (wasmi on the host), for checking the zkVM run.
//! START is a libaw state, or an AWBW-style turn file (libaw-arena's maps/skirmish.json).
//!   cargo run --release -p libaw-turn --bin game-native -- START SEED MAX_DAYS FIRST.wasm SECOND.wasm
use libaw_turn::{play_game, GameInput};
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let state = libaw_turn::load_state(&std::fs::read(&args[1]).unwrap()).unwrap();
    let (seed, max_days) = (args[2].parse().unwrap(), args[3].parse().unwrap());
    let (a, b) = (std::fs::read(&args[4]).unwrap(), std::fs::read(&args[5]).unwrap());
    let input = GameInput { state, seed, max_days, max_actions: 400, fuel_per_action: 2_000_000_000 };
    let t = Instant::now();
    let (out, _, _) = play_game(input, [&a, &b], || 0).unwrap();
    println!("{} actions, {} turns in {:.3}s", out.actions, out.turns, t.elapsed().as_secs_f64());
    println!("{}", serde_json::to_string(&out).unwrap());
}
