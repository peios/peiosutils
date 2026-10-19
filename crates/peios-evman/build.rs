//! Use an explicitly installed development library, or the system linker path.

fn main() {
    println!("cargo:rerun-if-env-changed=PEIOS_EVMAN_LIB_DIR");
    match std::env::var("PEIOS_EVMAN_LIB_DIR") {
        Ok(directory) => println!("cargo:rustc-link-search=native={directory}"),
        Err(std::env::VarError::NotPresent) => {}
        Err(error) => panic!("PEIOS_EVMAN_LIB_DIR: {error}"),
    }
}
