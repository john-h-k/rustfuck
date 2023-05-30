use std::collections::VecDeque;
use std::io::Write;
use std::{io, mem, slice};

use anyhow::Result;
use dynasmrt::{dynasm, AssemblyOffset, DynasmApi, DynasmLabelApi};

use crate::lir::LirOp;

pub struct Jit;

impl Jit {
    pub fn jit(program: &[LirOp]) -> Result<()> {
        let mut branch_table = VecDeque::new();

        let mut asm = dynasmrt::x64::Assembler::new().unwrap();

        for op in program {
            match op {
                LirOp::Modify(delta) => {
                    let delta = i32::try_from(*delta).expect("insane bounds");
                    dynasm!(asm
                        ; .arch aarch64
                        ; ldrb w1, [x0]
                        ; add w1, w1, delta as u32
                        ; strb w1, [x0]
                    )
                }
                LirOp::Move(delta) => {
                    let delta = i32::try_from(*delta).expect("insane bounds");
                    dynasm!(asm
                        ; .arch aarch64
                        ; add x0, x0, delta as u32
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
                        ; ldrb w1, [x0]
                        ; cbnz w1, >end
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
                        ; ldrb w1, [x0]
                        ; ldrb w2, [x0, delta as u32]
                        ; add w1, w1, w2
                        ; strb w1, [x0, delta as u32]
                    )
                }
                LirOp::In => todo!("this is gonna be a pain"),
                LirOp::Out => dynasm!(asm
                    ; .arch aarch64
                    ; mov x1, x0
                    ; mov x2, #1
                    ; mov x16, #4
                    ; svc #0x80
                ),
                LirOp::BrFor => {
                    let back_branch = asm.new_dynamic_label();
                    let for_branch = asm.new_dynamic_label();

                    branch_table.push_back((for_branch, back_branch));

                    dynasm!(asm
                        ; .arch aarch64
                        ; ldrb w1, [x0]
                        ; cmp w1, #0
                        ; cbz w1, =>for_branch
                        ; .align 4
                        ; =>back_branch
                    )
                }
                LirOp::BrBack => {
                    let (for_branch, back_branch) =
                        branch_table.pop_back().expect("unmatched loop");

                    dynasm!(asm
                        ; .arch aarch64
                        ; ldrb w1, [x0]
                        ; cbnz w1, =>back_branch
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

        let func: extern "C" fn(*mut u8) -> () =
            unsafe { mem::transmute(func.ptr(AssemblyOffset(0))) };

        let mut cells = [0u8; 30_000];
        eprintln!("Executing raw!");
        func(cells.as_mut_ptr());
        eprintln!("Somehow finished");

        Ok(())
    }
}
