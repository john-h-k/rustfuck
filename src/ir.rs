use std::fmt;

pub trait IrLike {
    fn is_begin_branch(&self) -> bool;
    fn is_end_branch(&self) -> bool;

    fn to_compact(&self) -> String;
}

impl<T: IrLike> IrLike for [T] {
    // TODO: ugly af design with is_xxx_branch
    fn is_begin_branch(&self) -> bool {
        false
    }

    fn is_end_branch(&self) -> bool {
        false
    }

    fn to_compact(&self) -> String {
        let mut r = String::new();

        let mut depth = 0;

        for instr in self.iter() {
            if instr.is_end_branch() {
                depth -= 1;
            }

            r.push_str(&"  ".repeat(depth));
            r.push_str(&instr.to_compact());
            r.push('\n');

            if instr.is_begin_branch() {
                depth += 1;
            }
        }

        r
    }
}
