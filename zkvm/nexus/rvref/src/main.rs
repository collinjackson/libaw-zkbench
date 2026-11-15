//! A plain RV32IM interpreter that loads a Nexus guest ELF and implements the syscalls the
//! Harvard emulator does, so a guest's run can be compared with Nexus's, block by block.
//!   rvref <elf> <encoded private input> [nexus trace]
//! The trace is the host's TRACE output: "clock pc x1..x31" (hex) after each basic block. With
//! it, rvref stops at the first block end where its pc or registers differ and prints the
//! instructions that led there.
use std::collections::{HashMap, VecDeque};
use std::io::BufRead;

struct Mem(HashMap<u32, Box<[u8; 4096]>>);
impl Mem {
    fn page(&mut self, a: u32) -> &mut [u8; 4096] { self.0.entry(a >> 12).or_insert_with(|| Box::new([0; 4096])) }
    fn r8(&mut self, a: u32) -> u8 { self.page(a)[(a & 4095) as usize] }
    fn w8(&mut self, a: u32, v: u8) { self.page(a)[(a & 4095) as usize] = v }
    fn r(&mut self, a: u32, n: u32) -> u32 { (0..n).fold(0, |v, i| v | (self.r8(a.wrapping_add(i)) as u32) << (8 * i)) }
    fn w(&mut self, a: u32, n: u32, v: u32) { for i in 0..n { self.w8(a.wrapping_add(i), (v >> (8 * i)) as u8) } }
}

fn load(elf: &[u8], m: &mut Mem) -> u32 {
    let u32_at = |o: usize| u32::from_le_bytes(elf[o..o + 4].try_into().unwrap());
    let u16_at = |o: usize| u16::from_le_bytes(elf[o..o + 2].try_into().unwrap());
    let (entry, phoff, phentsize, phnum) = (u32_at(0x18), u32_at(0x1c) as usize, u16_at(0x2a) as usize, u16_at(0x2c) as usize);
    for i in 0..phnum {
        let p = phoff + i * phentsize;
        if u32_at(p) != 1 { continue } // PT_LOAD
        let (off, va, fs) = (u32_at(p + 4) as usize, u32_at(p + 8), u32_at(p + 16) as usize);
        for j in 0..fs { m.w8(va + j as u32, elf[off + j]) }
    }
    entry
}

fn sx(v: u32, bits: u32) -> u32 { (((v << (32 - bits)) as i32) >> (32 - bits)) as u32 }

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let elf = std::fs::read(&a[1]).unwrap();
    let mut input: VecDeque<u8> = std::fs::read(&a[2]).unwrap().into();
    let mut trace = a.get(3).map(|p| std::io::BufReader::new(std::fs::File::open(p).unwrap()).lines());
    let mut m = Mem(HashMap::new());
    let mut pc = load(&elf, &mut m);
    let mut x = [0u32; 32];
    let mut clock: u64 = 1; // Nexus counts from 1
    let mut want: Option<(u64, u32, Vec<u32>)> = None;
    let mut recent: VecDeque<(u32, u32)> = VecDeque::new();
    let next_want = |t: &mut Option<std::io::Lines<std::io::BufReader<std::fs::File>>>| -> Option<(u64, u32, Vec<u32>)> {
        let l = t.as_mut()?.next()?.ok()?;
        let mut f = l.split(' ');
        let c = f.next()?.parse().ok()?;
        let p = u32::from_str_radix(f.next()?, 16).ok()?;
        Some((c, p, f.map(|v| u32::from_str_radix(v, 16).unwrap()).collect()))
    };
    want = want.or_else(|| next_want(&mut trace));
    loop {
        let ins = m.r(pc, 4);
        recent.push_back((pc, ins));
        if recent.len() > 40 { recent.pop_front(); }
        let (op, rd, f3, rs1, rs2, f7) = (ins & 0x7f, (ins >> 7) & 31, (ins >> 12) & 7, (ins >> 15) & 31, (ins >> 20) & 31, ins >> 25);
        let (v1, v2) = (x[rs1 as usize], x[rs2 as usize]);
        let imm_i = sx(ins >> 20, 12);
        let imm_s = sx(((ins >> 25) << 5) | ((ins >> 7) & 31), 12);
        let imm_b = sx(((ins >> 31) << 12) | (((ins >> 7) & 1) << 11) | (((ins >> 25) & 63) << 5) | (((ins >> 8) & 15) << 1), 13);
        let imm_j = sx(((ins >> 31) << 20) | (((ins >> 12) & 255) << 12) | (((ins >> 20) & 1) << 11) | (((ins >> 21) & 1023) << 1), 21);
        let mut next = pc.wrapping_add(4);
        let mut out: Option<u32> = None;
        match op {
            0x37 => out = Some(ins & 0xffff_f000),
            0x17 => out = Some(pc.wrapping_add(ins & 0xffff_f000)),
            0x6f => { out = Some(next); next = pc.wrapping_add(imm_j) }
            0x67 => { out = Some(next); next = v1.wrapping_add(imm_i) & !1 }
            0x63 => {
                let t = match f3 { 0 => v1 == v2, 1 => v1 != v2, 4 => (v1 as i32) < (v2 as i32), 5 => (v1 as i32) >= (v2 as i32), 6 => v1 < v2, 7 => v1 >= v2, _ => panic!("bad branch at {pc:#x}") };
                if t { next = pc.wrapping_add(imm_b) }
            }
            0x03 => {
                let ad = v1.wrapping_add(imm_i);
                out = Some(match f3 { 0 => sx(m.r(ad, 1), 8), 1 => sx(m.r(ad, 2), 16), 2 => m.r(ad, 4), 4 => m.r(ad, 1), 5 => m.r(ad, 2), _ => panic!("bad load at {pc:#x}") });
            }
            0x23 => { let ad = v1.wrapping_add(imm_s); m.w(ad, [1, 2, 4][f3 as usize], v2) }
            0x13 => {
                let sh = (ins >> 20) & 31;
                out = Some(match f3 {
                    0 => v1.wrapping_add(imm_i), 2 => ((v1 as i32) < (imm_i as i32)) as u32, 3 => (v1 < imm_i) as u32,
                    4 => v1 ^ imm_i, 6 => v1 | imm_i, 7 => v1 & imm_i, 1 => v1 << sh,
                    5 => if f7 & 0x20 != 0 { ((v1 as i32) >> sh) as u32 } else { v1 >> sh },
                    _ => unreachable!(),
                });
            }
            0x33 if f7 == 1 => {
                let (s1, s2) = (v1 as i32 as i64, v2 as i32 as i64);
                out = Some(match f3 {
                    0 => v1.wrapping_mul(v2),
                    1 => ((s1 * s2) >> 32) as u32,
                    2 => ((s1 * v2 as i64) >> 32) as u32,
                    3 => ((v1 as u64 * v2 as u64) >> 32) as u32,
                    4 => if v2 == 0 { u32::MAX } else if v1 == 0x8000_0000 && v2 == u32::MAX { v1 } else { ((v1 as i32) / (v2 as i32)) as u32 },
                    5 => if v2 == 0 { u32::MAX } else { v1 / v2 },
                    6 => if v2 == 0 { v1 } else if v1 == 0x8000_0000 && v2 == u32::MAX { 0 } else { ((v1 as i32) % (v2 as i32)) as u32 },
                    7 => if v2 == 0 { v1 } else { v1 % v2 },
                    _ => unreachable!(),
                });
            }
            0x33 => {
                out = Some(match (f3, f7) {
                    (0, 0) => v1.wrapping_add(v2), (0, 0x20) => v1.wrapping_sub(v2), (1, _) => v1 << (v2 & 31),
                    (2, _) => ((v1 as i32) < (v2 as i32)) as u32, (3, _) => (v1 < v2) as u32, (4, _) => v1 ^ v2,
                    (5, 0) => v1 >> (v2 & 31), (5, 0x20) => ((v1 as i32) >> (v2 & 31)) as u32, (6, _) => v1 | v2, (7, _) => v1 & v2,
                    _ => panic!("bad op at {pc:#x}"),
                });
            }
            0x0f => {} // fence
            0x73 => {
                let r = match x[17] {
                    0x200 => {
                        if x[10] == 1 { let s: Vec<u8> = (0..x[12]).map(|i| m.r8(x[11] + i)).collect(); print!("{}", String::from_utf8_lossy(&s)); x[12] } else { u32::MAX }
                    }
                    0x201 => { println!("rvref: exit {} after {} cycles", x[10], clock - 1); return }
                    0x400 => input.pop_front().map_or(u32::MAX, |b| b as u32),
                    0x401 => x[10], // cycle-count markers leave a0 alone
                    0x403 => 0,
                    _ => u32::MAX, // stack pointer and heap advise: Nexus leaves its default
                };
                x[10] = r;
            }
            _ => panic!("unknown instruction {ins:#010x} at {pc:#x}"),
        }
        if let Some(v) = out { if rd != 0 { x[rd as usize] = v } }
        pc = next;
        clock += 1;
        if let Some((c, p, r)) = &want {
            if *c == clock {
                if *p != pc || r.as_slice() != &x[1..] {
                    println!("first difference after block ending at clock {clock}: nexus pc {p:#x}, rvref pc {pc:#x}");
                    for i in 1..32 { if r[i - 1] != x[i] { println!("  x{i}: nexus {:#x}, rvref {:#x}", r[i - 1], x[i]) } }
                    println!("last instructions (pc, word):");
                    for (p, w) in &recent { println!("  {p:#07x} {w:08x}") }
                    return;
                }
                want = next_want(&mut trace);
            }
        }
        if clock > 50_000_000 { println!("rvref: no exit after 50M cycles, pc {pc:#x}"); return }
    }
}
