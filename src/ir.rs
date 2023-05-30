pub trait IrLike {
    fn to_compact(&self) -> String;
}

impl<T: IrLike> IrLike for [T] {
    fn to_compact(&self) -> String {
        let mut r = String::new();

        for instr in self.iter() {
            r.push_str(&instr.to_compact());
            r.push(' ');
        }

        r
    }
}
