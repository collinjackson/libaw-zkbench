// Runs a workload in OpenVM's metered executor (no proof), prints the instruction count and checks
// the output (length and FNV-1a-64 hash, the guest's public values) against the native run.
//   cargo run --release -p host-openvm -- ../workloads/turn.json
use openvm_build::GuestOptions;
use openvm_sdk::{config::AggregationSystemParams, Sdk, StdIn};
use openvm_stark_sdk::config::{app_params_with_100_bits_security, MAX_APP_LOG_STACKED_HEIGHT};
use std::time::Instant;

fn fnv(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf29ce484222325u64, |h, &x| (h ^ x as u64).wrapping_mul(0x100000001b3))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bytes = std::fs::read(&args[1]).unwrap();
    let native = serde_json::to_vec(&libaw_proof::run(serde_json::from_slice(&bytes).unwrap())).unwrap();
    let h = fnv(&native);
    let want: Vec<u8> = [native.len() as u32, h as u32, (h >> 32) as u32].iter().flat_map(|w| w.to_le_bytes()).collect();

    let sdk = Sdk::standard(app_params_with_100_bits_security(MAX_APP_LOG_STACKED_HEIGHT), AggregationSystemParams::default());
    let guest = concat!(env!("CARGO_MANIFEST_DIR"), "/../guest");
    let elf = sdk.build(GuestOptions::default(), guest, &None, None).unwrap();
    let mut stdin = StdIn::default();
    stdin.write_bytes(&bytes);
    let t = Instant::now();
    let (pv, (cost, instructions)) = sdk.execute_metered_cost(elf, stdin).unwrap();
    println!("{}: instructions {}, cost {}, execute {:.2}s, matches native: {}",
        args[1], instructions, cost, t.elapsed().as_secs_f64(), pv.len() >= 12 && pv[..12] == want[..]);
}
