use std::collections::VecDeque;
use std::io::Write;
use std::{io, mem, ptr, slice, u8};

use anyhow::Result;
use dynasmrt::{dynasm, AssemblyOffset, DynasmApi, DynasmLabelApi};
use tap::Tap;

use crate::lir::LirOp;

pub struct Jit;

impl Jit {
    pub fn jit(program: &[LirOp]) -> Result<()> {
        let mut branch_table = VecDeque::new();

        let mut asm = dynasmrt::aarch64::Assembler::new().unwrap();

        let split_4 = |val: &isize| {
            (
                *val as u16,
                val >> 16 as u32,
                val >> 32 as u16,
                val >> 48 as u16,
            )
        };

        for op in program {
            match op {
                LirOp::Modify(delta) => {
                    if delta > 0 {
                        dynasm!(asm
                            ; .arch aarch64
                            ; ldrb w2, [x0]
                            ; mov x3, delta as u64
                            ; add x2, x2, x3
                            ; and x2, x2, #0xFF
                            ; strb w2, [x0]
                        )
                    } else {
                        dynasm!(asm
                            ; .arch aarch64
                            ; ldrb w2, [x0]
                            ; mov x3, delta as u64
                            ; add x2, x2, x3
                            ; and x2, x2, #0xFF
                            ; strb w2, [x0]
                        )
                    }
                }
                LirOp::Move(delta) => {
                    dbg!(delta);
                    let delta = i64::try_from(*delta).expect("insane bounds");
                    dynasm!(asm
                        ; .arch aarch64
                        ; mov x3, delta as u64
                        ; add x0, x0, x3
                    )
                }
                LirOp::WriteZero => {
                    dynasm!(asm
                        ; .arch aarch64
                        ; strb wzr, [x0]
                    )
                }
                LirOp::Hop(delta) => {
                    let delta = i32::try_from(*delta).expect("insane bounds");
                    dynasm!(asm
                        ; .arch aarch64
                        ; .align 8
                        ; start:
                        ; ldrb w2, [x0]
                        ; cbnz w2, >end
                        ; add x0, x0, delta as u32
                        ; b >start
                        ; .align 8
                        ; end:
                    )
                }
                LirOp::MoveCell(delta) => {
                    let delta = i32::try_from(*delta).expect("insane bounds");
                    dynasm!(asm
                        ; .arch aarch64
                        ; ldrb w2, [x0]
                        ; ldrb w3, [x0, delta as u32]
                        ; add w2, w2, w3
                        ; and w2, w2, #0xFF
                        ; strb w2, [x0, delta as u32]
                    )
                }
                LirOp::In => {} //todo!("this is gonna be a pain"),
                LirOp::Out => dynasm!(asm
                    ; .arch aarch64
                    ; ldrb w2, [x0]
                    ; strb w2, [x1]
                    ; add x1, x1, #1
                ),
                LirOp::BrFor => {
                    let back_branch = asm.new_dynamic_label();
                    let for_branch = asm.new_dynamic_label();

                    branch_table.push_back((for_branch, back_branch));

                    dynasm!(asm
                        ; .arch aarch64
                        ; ldrb w2, [x0]
                        ; cbz w2, =>for_branch
                        ; .align 4
                        ; =>back_branch
                    )
                }
                LirOp::BrBack => {
                    let (for_branch, back_branch) =
                        branch_table.pop_back().expect("unmatched loop");

                    dynasm!(asm
                        ; .arch aarch64
                        ; ldrb w2, [x0]
                        ; cbnz w2, =>back_branch
                        ; .align 4
                        ; =>for_branch
                    )
                }
                LirOp::Meta(_) => { /* meta nodes ignored */ }
            }
        }

        assert!(branch_table.is_empty());

        dynasm!(asm
            ; .arch aarch64
            ; ret
        );

        let func = asm.finalize().expect("asm gen failed");

        let func: extern "C" fn(cells: *mut u8, buff: *mut u8) -> () =
            unsafe { mem::transmute(func.ptr(AssemblyOffset(0))) };

        let mut cells = [0u8; 30_000];
        let mut buff = [0u8; 30_000];
        eprintln!("Executing raw!");

        func(
            cells.as_mut_ptr().tap(|p| {
                dbg!(p);
            }),
            buff.as_mut_ptr(),
        );

        io::stdout().write_all(&buff[0..buff.iter().position(|&b| b == 0).unwrap()])?;

        eprintln!("Somehow finished");

        Ok(())
    }
}
