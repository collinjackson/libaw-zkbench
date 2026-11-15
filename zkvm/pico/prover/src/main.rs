// Runs a workload in Pico's emulator (no proof), prints cycles and checks the output against the
// native run.
//   (cd ../app && cargo pico build) && cargo run --release -- ../../workloads/turn.json
use pico_prover::{Prover, Stdin};
use std::time::Instant;

const ELF_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../app/elf/riscv64im-pico-zkvm-elf");

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bytes = std::fs::read(&args[1]).unwrap();
    let native = serde_json::to_vec(&libaw_proof::run(serde_json::from_slice(&bytes).unwrap())).unwrap();

    let elf = std::fs::read(ELF_PATH).unwrap();
    let mut stdin = Stdin::new();
    stdin.write_bytes(&bytes);
    let t = Instant::now();
    let run = Prover::builder(&elf).execute_chunked(stdin).unwrap();
    println!(
        "{}: cycles {}, execute {:.2}s, matches native: {}",
        args[1], run.cycles, t.elapsed().as_secs_f64(), run.committed_bytes == native
    );
}
