// Proving for the turn and game hosts: a composite proof (one STARK per segment), its
// compression to one succinct STARK, and optionally that wrapped as Groth16 (x86 Linux with
// Docker only), each timed and verified. Writes the last receipt to RECEIPT if given.
use risc0_zkvm::{default_prover, ExecutorEnv, InnerReceipt, ProverOpts, Receipt};
use std::time::Instant;

pub struct Opts {
    pub prove: bool,
    pub groth16: bool,
    pub receipt: Option<String>,
}

impl Opts {
    pub fn from_args(args: &mut Vec<String>) -> Opts {
        let mut o = Opts { prove: false, groth16: false, receipt: None };
        let mut rest = vec![];
        let mut it = std::mem::take(args).into_iter();
        while let Some(a) = it.next() {
            match a.as_str() {
                "--prove" => o.prove = true,
                "--groth16" => (o.prove, o.groth16) = (true, true),
                "--receipt" => o.receipt = it.next(),
                _ => rest.push(a),
            }
        }
        *args = rest;
        o
    }
}

fn size(r: &Receipt) -> usize {
    bincode::serialize(r).unwrap().len()
}

pub fn run(o: &Opts, input: &[u8], elf: &[u8], id: [u32; 8]) {
    if !o.prove {
        return;
    }
    let prover = default_prover();
    let env = ExecutorEnv::builder().write_slice(input).build().unwrap();
    let t = Instant::now();
    let info = prover.prove_with_opts(env, elf, &ProverOpts::composite()).unwrap();
    let secs = t.elapsed().as_secs_f64();
    let r = info.receipt;
    let segs = match &r.inner {
        InnerReceipt::Composite(c) => c.segments.len(),
        _ => 0,
    };
    r.verify(id).unwrap();
    println!("prove composite: {secs:.1}s, {segs} segments, total cycles {}, receipt {} bytes, verified", info.stats.total_cycles, size(&r));

    let t = Instant::now();
    let r = prover.compress(&ProverOpts::succinct(), &r).unwrap();
    let secs = t.elapsed().as_secs_f64();
    r.verify(id).unwrap();
    println!("compress to succinct: {secs:.1}s, receipt {} bytes, verified", size(&r));

    let r = if o.groth16 {
        let t = Instant::now();
        let g = prover.compress(&ProverOpts::groth16(), &r).unwrap();
        let secs = t.elapsed().as_secs_f64();
        let t = Instant::now();
        g.verify(id).unwrap();
        let v = t.elapsed().as_secs_f64();
        let seal = g.inner.groth16().unwrap().seal.len();
        println!(
            "wrap to groth16: {secs:.1}s, seal {seal} bytes, journal {} bytes, receipt {} bytes, verify {:.3}s, verified",
            g.journal.bytes.len(), size(&g), v
        );
        g
    } else {
        r
    };
    if let Some(p) = &o.receipt {
        std::fs::write(p, bincode::serialize(&r).unwrap()).unwrap();
        println!("receipt written to {p}");
    }
}
