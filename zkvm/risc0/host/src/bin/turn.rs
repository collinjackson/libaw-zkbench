// Runs the turn prover guest in RISC Zero's executor for one bot on a workload's start position,
// prints cycles and the split, and checks the turn against a native run; with --prove (or
// --groth16) proves it too (src/prove.rs).
//   cargo run --release -p host --bin turn -- ../workloads/turn.json BOT.wasm [MAX_ACTIONS] [--prove|--groth16] [--receipt FILE]
use methods::{TURN_ELF, TURN_ID};
use risc0_zkvm::{default_executor, ExecutorEnv};
use std::time::Instant;

#[path = "../prove.rs"]
mod prove;

fn main() {
    let mut args: Vec<String> = std::env::args().collect();
    let opts = prove::Opts::from_args(&mut args);
    let w: libaw_proof::Input = serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let wasm = std::fs::read(&args[2]).unwrap();
    let max_actions = args.get(3).map(|s| s.parse().unwrap()).unwrap_or(400);
    let input = libaw_turn::Input { state: w.state, seed: w.seed, max_actions, fuel_per_action: 2_000_000_000 };
    let json = serde_json::to_vec(&input).unwrap();
    let mut bytes = (json.len() as u32).to_le_bytes().to_vec();
    bytes.extend_from_slice(&json);
    bytes.extend_from_slice(&wasm);

    let (native, _) = libaw_turn::play_turn(serde_json::from_slice(&json).unwrap(), &wasm, || 0).unwrap();

    let env = ExecutorEnv::builder().write_slice(&bytes).build().unwrap();
    let t = Instant::now();
    let info = default_executor().execute(env, TURN_ELF).unwrap();
    let secs = t.elapsed().as_secs_f64();
    let journal: serde_json::Value = serde_json::from_slice(&info.journal.bytes).unwrap();
    let matches = journal["turn"] == serde_json::to_value(&native).unwrap();
    println!(
        "{} ({} bytes): {} actions, user cycles {}, total cycles {}, segments {}, execute {secs:.2}s, matches native: {matches}",
        args[2], wasm.len(), native.moves.len(), info.cycles(), info.segments.iter().map(|s| 1u64 << s.po2).sum::<u64>(), info.segments.len(),
    );
    println!("module sha256 {}", journal["module_sha256"]);
    prove::run(&opts, &bytes, TURN_ELF, TURN_ID);
}
