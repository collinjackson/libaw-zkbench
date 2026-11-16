// Runs the game guest (a whole game between two bots) in RISC Zero's executor, prints cycles and
// checks the result against a native run; with --prove (or --groth16) proves it too
// (src/prove.rs). START is a libaw state or an AWBW-style turn file (libaw-arena's
// maps/skirmish.json); the first module plays the first seat.
//   cargo run --release -p host --bin game -- START SEED MAX_DAYS FIRST.wasm SECOND.wasm [--prove|--groth16] [--receipt FILE]
use methods::{GAME_ELF, GAME_ID};
use risc0_zkvm::{default_executor, ExecutorEnv};
use std::time::Instant;

#[path = "../prove.rs"]
mod prove;

fn main() {
    let mut args: Vec<String> = std::env::args().collect();
    let opts = prove::Opts::from_args(&mut args);
    let state = libaw_turn::load_state(&std::fs::read(&args[1]).unwrap()).unwrap();
    let (seed, max_days) = (args[2].parse().unwrap(), args[3].parse().unwrap());
    let (a, b) = (std::fs::read(&args[4]).unwrap(), std::fs::read(&args[5]).unwrap());
    let input = libaw_turn::GameInput { state, seed, max_days, max_actions: 400, fuel_per_action: 2_000_000_000 };
    let json = serde_json::to_vec(&input).unwrap();
    let mut bytes = (json.len() as u32).to_le_bytes().to_vec();
    bytes.extend_from_slice(&json);
    bytes.extend_from_slice(&(a.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&a);
    bytes.extend_from_slice(&b);

    let t = Instant::now();
    let (native, _, _) = libaw_turn::play_game(serde_json::from_slice(&json).unwrap(), [&a, &b], || 0).unwrap();
    let native_secs = t.elapsed().as_secs_f64();

    let env = ExecutorEnv::builder().write_slice(&bytes).build().unwrap();
    let t = Instant::now();
    let info = default_executor().execute(env, GAME_ELF).unwrap();
    let secs = t.elapsed().as_secs_f64();
    let journal: serde_json::Value = serde_json::from_slice(&info.journal.bytes).unwrap();
    let matches = journal["game"] == serde_json::to_value(&native).unwrap();
    println!(
        "{} vs {}, {max_days} days: {} actions, {} turns, user cycles {}, total cycles {}, segments {}, native {native_secs:.3}s, execute {secs:.2}s, matches native: {matches}",
        args[4], args[5], native.actions, native.turns, info.cycles(), info.segments.iter().map(|s| 1u64 << s.po2).sum::<u64>(), info.segments.len(),
    );
    println!("journal {}", journal);
    prove::run(&opts, &bytes, GAME_ELF, GAME_ID);
}
