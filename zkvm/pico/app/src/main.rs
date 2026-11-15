// The libaw referee program (crates/proof) as a Pico guest: input JSON in, output JSON committed
// as the public values.
#![no_main]
pico_sdk::entrypoint!(main);

pub fn main() {
    let buf = pico_sdk::io::read_vec();
    let out = libaw_proof::run(serde_json::from_slice(&buf).unwrap());
    pico_sdk::io::commit_bytes(&serde_json::to_vec(&out).unwrap());
}
