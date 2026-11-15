// The libaw referee program (crates/proof) as a RISC Zero guest: input JSON in, output JSON
// committed to the journal.
use risc0_zkvm::guest::env;
use std::io::Read;

fn main() {
    let mut buf = Vec::new();
    env::stdin().read_to_end(&mut buf).unwrap();
    let out = libaw_proof::run(serde_json::from_slice(&buf).unwrap());
    env::commit_slice(&serde_json::to_vec(&out).unwrap());
}
