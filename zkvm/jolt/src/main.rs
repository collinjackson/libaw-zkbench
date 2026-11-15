// Runs a workload through Jolt's tracer (no proof), prints the trace length (cycles) and checks the
// output against the native run. Jolt's analyze keeps the whole trace in memory, so the game
// workload (billions of cycles) is out of reach this way.
//   cargo run --release -- ../workloads/turn.json
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bytes = std::fs::read(&args[1]).unwrap();
    let native = serde_json::to_vec(&libaw_proof::run(serde_json::from_slice(&bytes).unwrap())).unwrap();
    let t = Instant::now();
    let summary = guest::analyze_referee(bytes);
    let out: Vec<u8> = jolt_sdk::postcard::from_bytes(&summary.io_device.outputs).unwrap_or_default();
    println!("{}: cycles {}, trace {:.2}s, matches native: {}", args[1], summary.trace_len(), t.elapsed().as_secs_f64(), out == native);
}
