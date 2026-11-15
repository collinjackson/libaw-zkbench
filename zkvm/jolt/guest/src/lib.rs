// The libaw referee program (crates/proof) as a Jolt guest: input JSON in, output JSON out.
#[jolt::provable(heap_size = 268435456, stack_size = 4194304, max_input_size = 262144, max_output_size = 4096, max_trace_length = 1073741824)]
fn referee(input: Vec<u8>) -> Vec<u8> {
    let out = libaw_proof::run(serde_json::from_slice(&input).unwrap());
    serde_json::to_vec(&out).unwrap()
}
