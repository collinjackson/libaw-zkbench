// Runs a workload in RISC Zero's executor (no proof unless --prove), prints cycles and checks the
// output against the native run.
//   cargo run --release -p host -- ../workloads/turn.json [--prove]
use methods::{REFEREE_ELF, REFEREE_ID};
use risc0_zkvm::{default_executor, default_prover, ExecutorEnv};
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bytes = std::fs::read(&args[1]).unwrap();
    let native = libaw_proof::run(serde_json::from_slice(&bytes).unwrap());
    let native = serde_json::to_vec(&native).unwrap();

    let env = ExecutorEnv::builder().write_slice(&bytes).build().unwrap();
    let t = Instant::now();
    let info = default_executor().execute(env, REFEREE_ELF).unwrap();
    let matches = info.journal.bytes == native;
    println!(
        "{}: user cycles {}, total cycles {}, segments {}, execute {:.2}s, matches native: {}",
        args[1], info.cycles(), info.segments.iter().map(|s| 1u64 << s.po2).sum::<u64>(), info.segments.len(),
        t.elapsed().as_secs_f64(), matches
    );
    println!("output: {}", String::from_utf8_lossy(&info.journal.bytes));

    if args.iter().any(|a| a == "--prove") {
        let env = ExecutorEnv::builder().write_slice(&bytes).build().unwrap();
        let t = Instant::now();
        let receipt = default_prover().prove(env, REFEREE_ELF).unwrap().receipt;
        let prove = t.elapsed().as_secs_f64();
        let t = Instant::now();
        receipt.verify(REFEREE_ID).unwrap();
        let size = bincode_len(&receipt);
        println!("prove {prove:.1}s, verify {:.3}s, receipt {size} bytes", t.elapsed().as_secs_f64());
    }
}

fn bincode_len(r: &risc0_zkvm::Receipt) -> usize {
    serde_json::to_vec(r).map(|v| v.len()).unwrap_or(0)
}
