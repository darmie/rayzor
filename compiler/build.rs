fn main() {
    // On Linux, export symbols for dynamically loaded shared libraries
    if cfg!(target_os = "linux") {
        println!("cargo:rustc-link-arg=-Wl,--export-dynamic");
    }
}
