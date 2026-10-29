//! Name the shared library by its soname, as the packages install it.

fn main() {
    println!("cargo:rustc-link-arg-cdylib=-Wl,-soname,libevman.so.1");
}
