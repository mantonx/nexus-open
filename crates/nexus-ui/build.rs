//! Compiles the .slint UI files into Rust at build time.

fn main() {
    slint_build::compile("ui/app.slint").expect("ui/app.slint failed to compile");
}
