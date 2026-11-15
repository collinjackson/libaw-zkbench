// Runs a workload in Nexus's Harvard emulator (no proof, no trace), prints cycles and checks
// the output against the native run. The SDK's run_with_input builds the whole proving trace,
// so this drives the emulator directly to count cycles cheaply.
//   (cd ../guest && cargo build --release) && cargo run --release -- ../../workloads/turn.json
use nexus_sdk::Viewable;
use nexus_vm::elf::ElfFile;
use nexus_vm::emulator::{Emulator, HarvardEmulator};
use std::time::Instant;
use nexus_common::cpu::Registers;

const ELF_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../guest/target/riscv32im-unknown-none-elf/release/referee-nexus"
);

// The SDK's encoding (Prover::encode_input): postcard COBS, zero-padded to a word.
fn encode<T: serde::Serialize>(v: &T) -> Vec<u8> {
    let mut e = postcard::to_stdvec_cobs(v).unwrap();
    e.resize((e.len() + 3) & !3, 0);
    e
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bytes = std::fs::read(&args[1]).unwrap();
    let native = serde_json::to_vec(&libaw_proof::run(serde_json::from_slice(&bytes).unwrap())).unwrap();

    let elf_path = std::env::var("NEXUS_ELF").unwrap_or(ELF_PATH.to_string());
    let elf = ElfFile::from_path(&elf_path).unwrap();
    let t = Instant::now();
    let input = encode(&bytes);
    if let Ok(p) = std::env::var("DUMP_INPUT") { std::fs::write(p, &input).unwrap(); }
    let mut emu = HarvardEmulator::from_elf(&elf, &[], &input);
    // TRACE=path: after every basic block, write "clock pc x1..x31" (hex) for diffing against
    // a reference interpreter.
    let mut trace = std::env::var("TRACE").ok().map(|p| std::io::BufWriter::new(std::fs::File::create(p).unwrap()));
    // `Emulator::execute` keeps every instruction's result and memory records until the run
    // ends (memory grows with cycles). Stepping block by block and dropping them keeps memory
    // flat; MAX_CYCLES (default 200M) stops a run that doesn't end.
    let max: usize = std::env::var("MAX_CYCLES").ok().and_then(|v| v.parse().ok()).unwrap_or(200_000_000);
    let mut next_report = 50_000_000;
    let res: Result<(), nexus_vm::error::VMError> = loop {
        let pc = emu.get_executor().cpu.pc.value;
        let block = match emu.fetch_block(pc) { Ok(b) => b, Err(e) => break Err(e) };
        if let Err(e) = emu.execute_basic_block(&block, false) { break Err(e) }
        let clk = emu.get_executor().global_clock;
        if let Some(w) = trace.as_mut() {
            use std::io::Write;
            let regs = &emu.get_executor().cpu.registers;
            let r: Vec<String> = (1u8..32).map(|i| format!("{:x}", regs.read(nexus_common::riscv::register::Register::from(i)))).collect();
            writeln!(w, "{} {:x} {}", clk, emu.get_executor().cpu.pc.value, r.join(" ")).unwrap();
        }
        if clk >= next_report {
            eprintln!("  {}M cycles, pc {:#x}, {:.1}s", clk / 1_000_000, pc, t.elapsed().as_secs_f64());
            next_report += 50_000_000;
        }
        if clk >= max {
            println!("{}: stopped at {} cycles (MAX_CYCLES), pc {:#x}, no exit", args[1], clk, pc);
            let regs = &emu.get_executor().cpu.registers;
            let names = ["zero","ra","sp","gp","tp","t0","t1","t2","s0","s1","a0","a1","a2","a3","a4","a5","a6","a7","s2","s3","s4","s5","s6","s7","s8","s9","s10","s11","t3","t4","t5","t6"];
            let line: Vec<String> = (1u8..32).map(|i| format!("{}={:#x}", names[i as usize], regs.read(nexus_common::riscv::register::Register::from(i)))).collect();
            println!("  {}", line.join(" "));
            return;
        }
    };
    let cycles = emu.get_executor().global_clock - 1;
    let view = emu.finalize();
    if std::env::var("NEXUS_ELF").is_ok() {
        println!("{elf_path}: exited after {cycles} cycles, exit code {:?}, output {:?}", view.exit_code(), view.public_output::<u32>());
        return;
    }
    if let Ok(logs) = view.logs() { for l in logs { eprint!("  guest: {l}"); } }
    let out: Vec<u8> = view.public_output().unwrap_or_else(|e| panic!("no output ({e:?}); run ended {:?}", res.err().map(|e| e.source)));
    println!(
        "{}: cycles {}, execute {:.2}s, matches native: {}",
        args[1], cycles, t.elapsed().as_secs_f64(), out == native
    );
}
