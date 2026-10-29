//! Use an explicitly installed development library, or the system linker path.

fn main() {
    println!("cargo:rerun-if-env-changed=PEIOS_REG_LIB_DIR");
    match std::env::var("PEIOS_REG_LIB_DIR") {
        Ok(directory) => {
            println!("cargo:rustc-link-search=native={directory}");
            // libreg needs libpeios, which a cross linker finds only by
            // `-rpath-link`, not by the search path above.
            println!("cargo:rustc-link-arg=-Wl,-rpath-link,{directory}");
        }
        Err(std::env::VarError::NotPresent) => {}
        Err(error) => panic!("PEIOS_REG_LIB_DIR: {error}"),
    }
}
