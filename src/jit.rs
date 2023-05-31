use std::collections::VecDeque;
use std::io::Write;
use std::{io, mem, u8};

use anyhow::Result;
use dynasmrt::{dynasm, AssemblyOffset, DynasmApi, DynasmLabelApi};
use log::trace;

use crate::ir::IrLike;
use crate::lir::LirOp;

pub struct Jit;

impl Jit {
    pub fn jit(program: &[LirOp]) -> Result<()> {
        trace!("Jitting Lir: {}", program.to_compact());

        let mut branch_table = VecDeque::new();

        let mut asm = dynasmrt::aarch64::Assembler::new().unwrap();

        for op in program {
            match op {
                op @ LirOp::Modify(delta) | op @ LirOp::OffsetModify(delta, ..) => {
                    // hacky af
                    let offset = if let LirOp::OffsetModify(_, offset) = op {
                        *offset as i64
                    } else {
                        0
                    };

                    let abs_offset = offset.unsigned_abs();

                    match offset {
                        1.. => dynasm!(asm
                            ; .arch aarch64
                            ; mov x4, abs_offset
                            ; add x5, x0, x4
                        ),
                        ..=-1 => dynasm!(asm
                            ; .arch aarch64
                            ; mov x4, abs_offset
                            ; sub x5, x0, x4
                        ),
                        _ => dynasm!(asm
                            ; .arch aarch64
                            ; mov x5, x0
                        ),
                    }

                    let abs_delta = (*delta as i64).unsigned_abs();
                    if *delta > 0 {
                        dynasm!(asm
                            ; .arch aarch64
                            ; ldrb w2, [x5]
                            ; mov x3, abs_delta
                            ; add x2, x2, x3
                            ; strb w2, [x5]
                        )
                    } else {
                        dynasm!(asm
                            ; .arch aarch64
                            ; ldrb w2, [x5]
                            ; mov x3, abs_delta
                            ; sub x2, x2, x3
                            ; strb w2, [x5]
                        )
                    }
                }
                LirOp::Move(delta) => {
                    let abs_delta = (*delta as i64).unsigned_abs();

                    if *delta > 0 {
                        dynasm!(asm
                            ; .arch aarch64
                            ; mov x3, abs_delta
                            ; add x0, x0, x3
                        )
                    } else {
                        dynasm!(asm
                            ; .arch aarch64
                            ; mov x3, abs_delta
                            ; sub x0, x0, x3
                        )
                    }
                }
                LirOp::WriteZero => {
                    dynasm!(asm
                        ; .arch aarch64
                        ; strb wzr, [x0]
                    )
                }
                LirOp::Hop(delta) => {
                    let abs_delta = (*delta as i64).unsigned_abs();

                    if *delta > 0 {
                        dynasm!(asm
                            ; .arch aarch64
                            ; mov w3, abs_delta
                            ; start:
                            ; ldrb w2, [x0]
                            ; cbz w2, >end
                            ; add x0, x0, x3
                            ; b <start
                            ; end:
                        )
                    } else {
                        dynasm!(asm
                            ; .arch aarch64
                            ; mov w3, abs_delta
                            ; start:
                            ; ldrb w2, [x0]
                            ; cbz w2, >end
                            ; sub x0, x0, x3
                            ; b <start
                            ; end:
                        )
                    }
                }
                LirOp::MoveCell(delta) => {
                    let abs_delta = (*delta as i64).unsigned_abs();

                    dynasm!(asm
                        ; .arch aarch64
                        ; mov w4, abs_delta
                    );

                    if *delta > 0 {
                        dynasm!(asm
                            ; .arch aarch64
                            ; add x5, x0, x4
                        )
                    } else {
                        dynasm!(asm
                            ; .arch aarch64
                            ; sub x5, x0, x4
                        )
                    }

                    dynasm!(asm
                        ; .arch aarch64
                        ; ldrb w2, [x0]
                        ; cbz w2, >skip
                        ; strb wzr, [x0]
                        ; ldrb w3, [x5]
                        ; add w2, w2, w3
                        ; and w2, w2, #0xFF
                        ; strb w2, [x5]
                        ; skip:
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

        func(cells.as_mut_ptr(), buff.as_mut_ptr());

        io::stdout().write_all(&buff[0..buff.iter().position(|&b| b == 0).unwrap()])?;

        Ok(())
    }
}
