fn main() {
    #[cfg(feature = "tcc-linker")]
    build_tcc();

    // On Linux, export symbols for dynamically loaded shared libraries
    if cfg!(target_os = "linux") {
        println!("cargo:rustc-link-arg=-Wl,--export-dynamic");
    }
}

#[cfg(feature = "tcc-linker")]
fn build_tcc() {
    let tcc_dir = std::path::Path::new("vendor/tinycc");
    if !tcc_dir.exists() {
        panic!("TCC source not found at vendor/tinycc. Run: git clone --depth 1 https://github.com/TinyCC/tinycc.git vendor/tinycc");
    }

    let mut build = cc::Build::new();
    build
        .file(tcc_dir.join("libtcc.c"))
        .include(tcc_dir)
        .define("ONE_SOURCE", "1")
        .define("TCC_LIBTCC", "1")
        .define("CONFIG_TCC_STATIC", "1")
        // Suppress warnings in vendored code
        .warnings(false);

    // Target-specific defines
    if cfg!(target_arch = "x86_64") {
        build.define("TCC_TARGET_X86_64", "1");
    } else if cfg!(target_arch = "aarch64") {
        build.define("TCC_TARGET_ARM64", "1");
    } else if cfg!(target_arch = "x86") {
        build.define("TCC_TARGET_I386", "1");
    }

    // Platform-specific
    if cfg!(target_os = "macos") {
        build.define("TCC_TARGET_MACHO", "1");
    }

    build.compile("tcc");

    println!("cargo:rerun-if-changed=vendor/tinycc/libtcc.c");
    println!("cargo:rerun-if-changed=vendor/tinycc/libtcc.h");
}
