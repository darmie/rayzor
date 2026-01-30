fn main() {
    #[cfg(feature = "tcc-runtime")]
    build_tcc();
}

#[cfg(feature = "tcc-runtime")]
fn build_tcc() {
    // TCC source lives in the compiler crate's vendor directory
    let tcc_dir = std::path::Path::new("../compiler/vendor/tinycc");
    if !tcc_dir.exists() {
        panic!(
            "TCC source not found at ../compiler/vendor/tinycc. \
             Run: git clone --depth 1 https://github.com/TinyCC/tinycc.git compiler/vendor/tinycc"
        );
    }

    // Resolve absolute path so TCC can find its own includes (tccdefs.h) at runtime
    let tcc_abs = std::fs::canonicalize(tcc_dir).expect("Failed to resolve TCC vendor path");
    let tcc_dir_quoted = format!("\"{}\"", tcc_abs.display());

    let mut build = cc::Build::new();
    build
        .file(tcc_dir.join("libtcc.c"))
        .include(tcc_dir)
        .define("ONE_SOURCE", "1")
        .define("TCC_LIBTCC", "1")
        .define("CONFIG_TCC_STATIC", "1")
        .define("CONFIG_TCCDIR", tcc_dir_quoted.as_str())
        .warnings(false);

    if cfg!(target_arch = "x86_64") {
        build.define("TCC_TARGET_X86_64", "1");
    } else if cfg!(target_arch = "aarch64") {
        build.define("TCC_TARGET_ARM64", "1");
    } else if cfg!(target_arch = "x86") {
        build.define("TCC_TARGET_I386", "1");
    }

    if cfg!(target_os = "macos") {
        build.define("TCC_TARGET_MACHO", "1");
    }

    build.compile("tcc");

    println!("cargo:rerun-if-changed=../compiler/vendor/tinycc/libtcc.c");
    println!("cargo:rerun-if-changed=../compiler/vendor/tinycc/libtcc.h");
}
