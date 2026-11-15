// The libaw referee program (crates/proof) as an OpenVM guest: input JSON in; the output JSON's
// length and FNV-1a-64 hash revealed as public values (OpenVM's public values are a fixed few
// words, so the host compares the hash with the native run's).
openvm::entry!(main);

fn fnv(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf29ce484222325u64, |h, &x| (h ^ x as u64).wrapping_mul(0x100000001b3))
}

fn main() {
    let buf = openvm::io::read_vec();
    let out = serde_json::to_vec(&libaw_proof::run(serde_json::from_slice(&buf).unwrap())).unwrap();
    let h = fnv(&out);
    openvm::io::reveal_u32(out.len() as u32, 0);
    openvm::io::reveal_u32(h as u32, 1);
    openvm::io::reveal_u32((h >> 32) as u32, 2);
}
