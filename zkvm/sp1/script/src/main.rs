// Runs a workload in SP1's executor (no proof), prints cycles and checks the output against the
// native run.
//   cargo run --release -- ../workloads/turn.json
use sp1_sdk::blocking::{ProverClient, Prover};
use sp1_sdk::{include_elf, Elf, SP1Stdin};
use std::time::Instant;

const ELF: Elf = include_elf!("referee-sp1");

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bytes = std::fs::read(&args[1]).unwrap();
    let native = serde_json::to_vec(&libaw_proof::run(serde_json::from_slice(&bytes).unwrap())).unwrap();

    let client = ProverClient::builder().cpu().build();
    let mut stdin = SP1Stdin::new();
    stdin.write_vec(bytes.clone());
    let t = Instant::now();
    let (public, report) = client.execute(ELF, stdin).run().unwrap();
    println!(
        "{}: cycles {}, execute {:.2}s, matches native: {}",
        args[1], report.total_instruction_count(), t.elapsed().as_secs_f64(), public.as_slice() == native.as_slice()
    );
}
