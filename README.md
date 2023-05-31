<h1 align="center">rustfuck</h1>
<div align="center">
 <strong>
  A simple brainfuck interpreter and REPL written in Rust
 </strong>
</div>

## Why?

I wanted to write a brainfuck interpreter. They are inherently quite useless

## What does it have?

`rustfuck` contains 3 interpreters as well as an optimising JIT-compiler (currently only available on AArch64).

The 4 backends:
* Raw interpreter - executes brainfuck, as-is
    * takes around 30s on `mandelbrot.b`
* HIR interpreter - only fuses `+-`s and `<>`s, as well as creating a branch lookup table
    * takes around 5s on `mandelbrot.b`
* LIR interpreter - additiionally performs a bunch of different loop optimisations to
    * takes around 2.7s on `mandelbrot.b`
* JIT compiler - emits raw machine code (doesn't currently support reading from STDIN)
    * takes around 600ms on `mandelbrot.b`

There are several examples in the `examples` folder, including `hello_world` and `mandelbrot`.