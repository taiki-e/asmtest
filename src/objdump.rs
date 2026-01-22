// SPDX-License-Identifier: Apache-2.0 OR MIT

// Refs:
// - https://llvm.org/docs/CommandGuide/llvm-objdump.html
// - https://sourceware.org/binutils/docs/binutils/objdump.html

use alloc::borrow::Cow;
use core::cmp;
use std::{collections::HashMap, sync::LazyLock};

use anyhow::Context as _;
use regex::Regex;

use crate::{ArchFamily, RevisionContext};

pub(crate) fn disassemble(cx: &mut RevisionContext<'_>) -> String {
    match cx.target_arch {
        // Always use GNU binutils for them because some instructions are not correctly recognized or dumped
        "avr" | "csky" | "m68k" | "msp430" | "mips" | "mips64" | "mips32r6" | "mips64r6"
        | "s390x" | "sparc" | "sparc64" | "xtensa" => {
            cx.prefer_gnu = true;
        }
        // hexagon is not supported in GNU binutils
        "hexagon" => cx.prefer_gnu = false,
        _ => {}
    }
    let mut objdump = cx.tcx.docker_cmd(cx.obj_path.parent().unwrap());
    objdump.args([
        if cx.prefer_gnu { "objdump" } else { "llvm-objdump" },
        "-Cd",
        "--disassembler-color=off",
    ]);
    objdump.arg(&cx.obj_path);
    match cx.target_arch {
        "mips" | "mips64" | "mips32r6" | "mips64r6" => {
            // TODO(mips)
            objdump.args(["-M", "reg-names=numeric"]);
        }
        "x86" | "x86_64" => {
            if cx.tcx.tester.config.att_syntax || cx.revision.config.att_syntax {
                objdump.args(["-M", "att"]);
            } else {
                objdump.args(["-M", "intel"]);
            }
        }
        _ => {}
    }
    objdump.args(&cx.tcx.tester.config.objdump_args);
    objdump.args(&cx.revision.config.objdump_args);
    objdump.read().unwrap()
}

pub(crate) fn handle_asm<'a>(cx: &mut RevisionContext<'a>, s: &'a str) {
    static FUNC_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new("\\n00000000+ <").unwrap());
    let mut label_map = HashMap::new();
    let mut lines = vec![];
    let mut func_iter = FUNC_RE.split(s);
    func_iter.next();
    for s in func_iter {
        let mut label_count = 0;
        label_map.clear();
        lines.clear();
        let (verbose_function_name, s) =
            s.split_once(">:\n").with_context(|| s.to_owned()).unwrap();
        let mut function_name = Cow::Borrowed(verbose_function_name);
        if !cx.prefer_gnu {
            if let Some((name, hash)) = verbose_function_name.rsplit_once("::") {
                // <path::to::fn::h[0-9a-f]{16}>:
                if hash.len() == 17
                    && hash.as_bytes()[0] == b'h'
                    && hash.as_bytes()[1..]
                        .iter()
                        .all(|&b| b.is_ascii_digit() | matches!(b, b'a'..=b'f'))
                {
                    cx.verbose_function_names.push(verbose_function_name);
                    function_name = Cow::Borrowed(name);
                }
            }
        }
        // TODO(hexagon,msp430,etc.): multiple spaces
        // https://github.com/taiki-e/atomic-maybe-uninit/blob/5e1cd2165c45e4362c6638b06b24fc37ea79884a/tests/asm-test/asm/atomic-maybe-uninit/hexagon.asm#L2346
        // https://github.com/taiki-e/atomic-maybe-uninit/blob/5e1cd2165c45e4362c6638b06b24fc37ea79884a/tests/asm-test/asm/atomic-maybe-uninit/msp430.asm#L2
        //
        // TODO(sparc): constant display bug:
        // https://github.com/taiki-e/atomic-maybe-uninit/blob/5e1cd2165c45e4362c6638b06b24fc37ea79884a/tests/asm-test/asm/atomic-maybe-uninit/sparcv8_leoncasa.asm#L1463
        // https://github.com/taiki-e/atomic-maybe-uninit/blob/5e1cd2165c45e4362c6638b06b24fc37ea79884a/tests/asm-test/asm/atomic-maybe-uninit/sparc64.asm#L564
        //
        //         mov               1, %i0	! 1 <asm_test::compare_exchange::u32::seqcst_relaxed+0x1>
        //         or                %i0, 0x3ff, %i5	! ffff <asm_test::compare_exchange::u16::acqrel_acquire+0xffff>
        //
        // The corresponding assembly is:
        //
        //         move_!($cc, "1", "{r}"),                    // if cc.Z { r = 1 }
        //         ?
        if cx.is_powerpc64be {
            if let Some(name) = function_name.strip_prefix(".text.") {
                // .text on big-endian PowerPC64 is not demangled by objdump 2.45.
                function_name = Cow::Owned(format!("{:#}", rustc_demangle::demangle(name)));
            }
        }
        if cx.arch_family == ArchFamily::Xtensa {
            // TODO(xtensa): .literal handling in Xtensa is not yet good:
            //
            //   .literal.asm_test::compare_exchange_weak::u16::seqcst_acquire:
            //           .byte             0xff
            //           .byte             0xff
            //
            //   asm_test::compare_exchange_weak::u16::seqcst_acquire:
            //           entry             a1, 32
            //           movi.n            a8, -4
            //           and               a8, a2, a8
            //           l32r              a9, fffc0008 <asm_test::compare_exchange_weak::u16::seqcst_acquire+0xfffc0008>
            //
            // Should be something like:
            //
            //   .literal.asm_test::compare_exchange_weak::u16::seqcst_acquire:
            //           .byte             0xff
            //           .byte             0xff
            //
            //   asm_test::compare_exchange_weak::u16::seqcst_acquire:
            //           entry             a1, 32
            //           movi.n            a8, -4
            //           and               a8, a2, a8
            //           l32r              a9, .literal.asm_test::compare_exchange_weak::u16::seqcst_acquire
            if let Some(name) = function_name.strip_prefix(".literal.") {
                // .literal is not demangled by objdump 2.45.
                function_name =
                    Cow::Owned(format!(".literal.{:#}", rustc_demangle::demangle(name)));
            }
        }
        let (label_re, addr_pos) = match cx.arch_family {
            ArchFamily::Arm if !cx.prefer_gnu => (
                format!(
                    "(-)?(0x)?[0-9a-f]+ <{verbose_function_name}(\\+0x([0-9a-f]+))?>( @ imm = #(-)?0x[0-9a-f]+)?"
                ),
                4,
            ),
            ArchFamily::Avr => (
                "\\.(\\+|-)[0-9]+ +\t; 0x([0-9a-f]+) <__zero_reg__(\\+0x[0-9a-f]+)?>".to_owned(),
                2,
            ),
            ArchFamily::CSky => (
                format!(
                    "0x[0-9a-f]+\t// (0x)?[0-9a-f]+ <{verbose_function_name}(\\+0x([0-9a-f]+))?>"
                ),
                3,
            ),
            ArchFamily::LoongArch if cx.prefer_gnu => (
                format!(
                    "(-)?(0x)?[0-9a-f]+\t# (-)?(0x)?[0-9a-f]+ <{verbose_function_name}(\\+0x([0-9a-f]+))?>"
                ),
                6,
            ),
            ArchFamily::Msp430 => ("\\$(\\+|-)[0-9]+ +\t;abs 0x([0-9a-f]+)".to_owned(), 2),
            _ => (format!("(-)?(0x)?[0-9a-f]+ <{verbose_function_name}(\\+0x([0-9a-f]+))?>"), 4),
        };
        let label_re = Regex::new(&label_re).unwrap();
        for c in label_re.captures_iter(s) {
            let addr = c.get(addr_pos).map_or("0", |m| m.as_str());
            let addr = u64::from_str_radix(addr, 16).with_context(|| addr.to_owned()).unwrap();
            label_map.insert(addr, None);
        }
        let mut line_iter = s.lines().peekable();
        while let Some(&s) = line_iter.peek() {
            if s.trim_ascii_start().is_empty() {
                line_iter.next();
                continue;
            }
            if s.starts_with(' ') {
                //  0: 89 f0                        <\t>mov	eax, esi
                // ^-- trim_ascii_start
                //   ^-- split_once(':')
                //    ^-- trim_ascii_start
                //                                  ^^^^-- split_once('\t')
                //          ^^^^^^^^^^^^^^^^^^^^^^^^-- trim_ascii_start
                //                                         ^-- split_once(['\t', ' '])
                if let Some((addr, s)) = s.trim_ascii_start().split_once(':') {
                    let addr =
                        u64::from_str_radix(addr, 16).with_context(|| addr.to_owned()).unwrap();
                    if let Some(n) = label_map.get_mut(&addr) {
                        *n = Some(label_count);
                        lines.push(Line::Label { num: label_count });
                        label_count += 1;
                    }
                    let s = s.trim_ascii_start();
                    let Some((_raw_insn, mut s)) = s.split_once('\t') else {
                        assert_eq!(cx.target_arch, "msp430");
                        for n in s.split([' ', '\t']) {
                            assert!(
                                n.is_empty()
                                    || n.len() == 2
                                        && n.as_bytes().iter().all(u8::is_ascii_hexdigit)
                            );
                        }
                        line_iter.next();
                        continue;
                    };
                    if cx.arch_family == ArchFamily::Hexagon {
                        //    8:<\t>e4 5f 00 78<\t>78005fe4   <\t>r4 = #0xff
                        //    8:<\t>e4 5f 00 78<\t>78005fe4 { <\t>r4 = #0xff
                        //                     ^^^^-- trim_ascii_start
                        //                                 ^-- split_once(' ')
                        //                                    ^^^^-- split_once('\t')
                        s = s.trim_ascii_start().split_once(' ').unwrap().1;
                        let (pre, s) = s.split_once('\t').unwrap();
                        lines.push(Line::Inst {
                            addr,
                            name: pre.trim_ascii(),
                            operands: s.trim_ascii().into(),
                        });
                    } else {
                        let (inst, operands) =
                            s.trim_ascii_start().split_once(['\t', ' ']).unwrap_or((s, ""));
                        lines.push(Line::Inst {
                            addr,
                            name: inst.trim_ascii_end(),
                            operands: operands.trim_ascii().into(),
                        });
                    }
                    line_iter.next();
                    continue;
                }
            }
            line_iter.next();
        }
        for line in &mut lines {
            let Line::Inst { addr: inst_addr, operands, .. } = line else { continue };
            let Cow::Borrowed(s) = *operands else { unreachable!() };
            *operands = label_re.replace_all(s, |c: &regex::Captures<'_>| {
                let addr = c.get(addr_pos).map_or("0", |m| m.as_str());
                let addr = u64::from_str_radix(addr, 16).with_context(|| addr.to_owned()).unwrap();
                if let Some(num) = label_map[&addr] {
                    if *inst_addr > addr { format!("{num}b") } else { format!("{num}f") }
                } else {
                    c.get(0).unwrap().as_str().to_owned()
                }
            });
        }
        write_func(cx, &function_name, &lines);
    }
    if !cx.verbose_function_names.is_empty() {
        let mut re = String::new();
        for &verbose_function_name in &cx.verbose_function_names {
            if !re.is_empty() {
                re.push('|');
            }
            re.push_str(verbose_function_name);
        }
        let re = Regex::new(&re).unwrap();
        if let Cow::Owned(new) = re.replace_all(&cx.out, |c: &regex::Captures<'_>| {
            c.get(0).unwrap().as_str().rsplit_once("::").unwrap().0.to_owned()
        }) {
            cx.out = new;
        }
    }
}

fn write_func(cx: &mut RevisionContext<'_>, function_name: &str, lines: &[Line<'_>]) {
    use core::fmt::Write as _;
    let _ = writeln!(cx.out, "{function_name}:");
    let mut instructions = lines.iter();
    while let Some(line) = instructions.next() {
        const START_PAD: &str = "        ";
        fn inst_pad(len: usize) -> &'static str {
            // We use 18 bytes as inst+pad length for now. The instruction with the longest name on
            // x86_64 is probably vgf2p8affineinvqb (17 bytes), so this should be sufficient in most cases.
            const MAX_INST_PAD: &str = "                  ";
            &MAX_INST_PAD[..cmp::max(MAX_INST_PAD.len().saturating_sub(len), 1)]
        }
        match *line {
            Line::Inst { addr: _, name: inst, ref operands } => {
                if cx.arch_family == ArchFamily::X86 && inst == "lock" {
                    if operands.is_empty() {
                        if let Some(Line::Inst { addr: _, name: inst, operands }) =
                            instructions.next()
                        {
                            let inst_pad = inst_pad(inst.len() + 5);
                            let _ = writeln!(cx.out, "{START_PAD}lock {inst}{inst_pad}{operands}");
                            continue;
                        }
                    } else {
                        let (inst, operands) = operands.split_once('\t').unwrap_or((operands, ""));
                        if operands.is_empty() {
                            let _ = writeln!(cx.out, "{START_PAD}lock {inst}");
                        } else {
                            let inst_pad = inst_pad(inst.len() + 5);
                            let _ = writeln!(cx.out, "{START_PAD}lock {inst}{inst_pad}{operands}");
                        }
                        continue;
                    }
                }
                if operands.is_empty() {
                    let _ = writeln!(cx.out, "{START_PAD}{inst}");
                } else if cx.arch_family == ArchFamily::Hexagon {
                    if inst.is_empty() {
                        let _ = writeln!(cx.out, "{START_PAD}  {operands}");
                    } else {
                        assert_eq!(inst, "{");
                        let _ = writeln!(cx.out, "{START_PAD}{{ {operands}");
                    }
                } else {
                    let inst_pad = inst_pad(inst.len());
                    let _ = writeln!(cx.out, "{START_PAD}{inst}{inst_pad}{operands}");
                }
            }
            Line::Label { num } => {
                let _ = writeln!(cx.out, "{num}:");
            }
        }
    }
    cx.out.push('\n');
}

enum Line<'a> {
    Inst { addr: u64, name: &'a str, operands: Cow<'a, str> },
    Label { num: u32 },
}
