// The libaw referee program (crates/proof) as an SP1 guest: input JSON in, output JSON committed
// as the public values.
#![no_main]
sp1_zkvm::entrypoint!(main);

pub fn main() {
    let buf = sp1_zkvm::io::read_vec();
    let out = libaw_proof::run(serde_json::from_slice(&buf).unwrap());
    sp1_zkvm::io::commit_slice(&serde_json::to_vec(&out).unwrap());
}
