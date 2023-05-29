use bumpalo::Bump;

pub struct IrGen {
    program: Vec<u8>,
}

pub enum IrOp {
    Inc,
    Dec,
    MvRight,
    MvLeft,
    In,
    Out,
}

pub enum IrLink<'ir> {
    Forward(&'ir IrBlock<'ir>),
    Back(&'ir IrBlock<'ir>),
    End
}

pub struct IrBlock<'ir> {
    ir: Vec<IrOp>,

    next: IrLink<'ir>,
}

impl IrGen {
    pub fn new(program: Vec<u8>) -> Self {
        Self { program }
    }

    pub fn gen(&mut self) -> () {
        let mut bump = Bump::new();

        let mut last_block = None;

        let mut ir = Vec::new();
        
        for command in self.program {
            match command {
                b'+' => ir.push(IrOp::Inc),
                b'-' => ir.push(IrOp::Dec),
                b'>' => ir.push(IrOp::MvRight),
                b'<' => ir.push(IrOp::MvLeft),
                b'.' => ir.push(IrOp::Out),
                b',' => ir.push(IrOp::In),

                br @ b'[' | br @ b']' => {
                    let next = match br => {
                        b'[' => IrLink::Forward()
                    }
                    let block = bump.alloc(IrBlock { ir })
                }

                _ => continue
            }
        }

        todo!()
    }
}
