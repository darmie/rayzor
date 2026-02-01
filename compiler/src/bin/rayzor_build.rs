//! rayzor-build: AOT compiler for Haxe → native executables
//!
//! Usage:
//!   rayzor-build [OPTIONS] <SOURCE_FILES...>
//!
//! Examples:
//!   rayzor-build -o hello hello.hx
//!   rayzor-build -O3 --target x86_64-unknown-linux-gnu -o hello hello.hx
//!   rayzor-build --emit llvm-ir -o hello.ll hello.hx

use compiler::codegen::aot_compiler::{AotCompiler, OutputFormat};
use compiler::ir::optimization::OptimizationLevel;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut compiler = AotCompiler::default();
    let mut output_path: Option<PathBuf> = None;
    let mut source_files: Vec<String> = Vec::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--output" | "-o" => {
                i += 1;
                if i < args.len() {
                    output_path = Some(PathBuf::from(&args[i]));
                }
            }
            "--target" => {
                i += 1;
                if i < args.len() {
                    compiler.target_triple = Some(args[i].clone());
                }
            }
            "--emit" => {
                i += 1;
                if i < args.len() {
                    compiler.output_format = match args[i].as_str() {
                        "exe" => OutputFormat::Executable,
                        "obj" => OutputFormat::ObjectFile,
                        "llvm-ir" => OutputFormat::LlvmIr,
                        "llvm-bc" => OutputFormat::LlvmBitcode,
                        "asm" => OutputFormat::Assembly,
                        other => {
                            eprintln!(
                                "Unknown emit format: {}. Use: exe, obj, llvm-ir, llvm-bc, asm",
                                other
                            );
                            std::process::exit(1);
                        }
                    };
                }
            }
            "--optimize" | "-O" => {
                i += 1;
                if i < args.len() {
                    compiler.opt_level = parse_opt_level(&args[i]);
                } else {
                    compiler.opt_level = OptimizationLevel::O2;
                }
            }
            "-O0" => compiler.opt_level = OptimizationLevel::O0,
            "-O1" => compiler.opt_level = OptimizationLevel::O1,
            "-O2" => compiler.opt_level = OptimizationLevel::O2,
            "-O3" => compiler.opt_level = OptimizationLevel::O3,
            "--no-strip" => compiler.strip = false,
            "--strip" => compiler.strip_symbols = true,
            "--runtime-dir" => {
                i += 1;
                if i < args.len() {
                    compiler.runtime_dir = Some(PathBuf::from(&args[i]));
                }
            }
            "--linker" => {
                i += 1;
                if i < args.len() {
                    compiler.linker = Some(args[i].clone());
                }
            }
            "--sysroot" => {
                i += 1;
                if i < args.len() {
                    compiler.sysroot = Some(PathBuf::from(&args[i]));
                }
            }
            "--verbose" | "-v" => compiler.verbose = true,
            "--help" | "-h" => {
                print_usage();
                return;
            }
            arg if !arg.starts_with('-') => {
                source_files.push(arg.to_string());
            }
            _ => {
                eprintln!("Warning: Unknown argument: {}", args[i]);
            }
        }
        i += 1;
    }

    if source_files.is_empty() {
        eprintln!("Error: No source files specified");
        eprintln!("Usage: rayzor-build [OPTIONS] <SOURCE_FILES...>");
        std::process::exit(1);
    }

    // Default output path
    let output = output_path.unwrap_or_else(|| {
        let base = PathBuf::from(&source_files[0])
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        match compiler.output_format {
            OutputFormat::Executable => {
                if cfg!(target_os = "windows")
                    || compiler
                        .target_triple
                        .as_deref()
                        .is_some_and(|t| t.contains("windows"))
                {
                    PathBuf::from(format!("{}.exe", base))
                } else {
                    PathBuf::from(&base)
                }
            }
            OutputFormat::ObjectFile => PathBuf::from(format!("{}.o", base)),
            OutputFormat::LlvmIr => PathBuf::from(format!("{}.ll", base)),
            OutputFormat::LlvmBitcode => PathBuf::from(format!("{}.bc", base)),
            OutputFormat::Assembly => PathBuf::from(format!("{}.s", base)),
        }
    });

    println!("Rayzor AOT Compiler");
    println!("  Sources: {}", source_files.join(", "));
    println!(
        "  Output:  {} ({:?})",
        output.display(),
        compiler.output_format
    );
    println!(
        "  Target:  {}",
        compiler.target_triple.as_deref().unwrap_or("host")
    );
    println!("  Opt:     {:?}", compiler.opt_level);
    println!();

    match compiler.compile(&source_files, &output) {
        Ok(result) => {
            println!();
            println!("Build succeeded:");
            println!(
                "  Output: {} ({} bytes)",
                result.path.display(),
                result.code_size
            );
            println!("  Target: {}", result.target_triple);
        }
        Err(e) => {
            eprintln!("Build failed: {}", e);
            std::process::exit(1);
        }
    }
}

fn parse_opt_level(s: &str) -> OptimizationLevel {
    match s {
        "0" => OptimizationLevel::O0,
        "1" => OptimizationLevel::O1,
        "2" => OptimizationLevel::O2,
        "3" => OptimizationLevel::O3,
        _ => {
            eprintln!("Warning: Unknown optimization level '{}', using O2", s);
            OptimizationLevel::O2
        }
    }
}

fn print_usage() {
    println!("rayzor-build: Compile Haxe to native executables");
    println!();
    println!("USAGE:");
    println!("    rayzor-build [OPTIONS] <SOURCE_FILES...>");
    println!();
    println!("OPTIONS:");
    println!("    -o, --output <FILE>       Output path (default: <source>.out)");
    println!("    --target <TRIPLE>         Target triple (default: host)");
    println!(
        "    --emit <FORMAT>           Output: exe, obj, llvm-ir, llvm-bc, asm (default: exe)"
    );
    println!("    -O0, -O1, -O2, -O3       Optimization level (default: O2)");
    println!("    --no-strip                Disable dead-code stripping");
    println!("    --strip                   Strip debug symbols from binary");
    println!("    --runtime-dir <DIR>       Path to librayzor_runtime.a");
    println!("    --linker <PATH>           Override linker path");
    println!("    --sysroot <PATH>          Sysroot for cross-compilation");
    println!("    -v, --verbose             Verbose output");
    println!("    -h, --help                Show this help message");
    println!();
    println!("EXAMPLES:");
    println!("    rayzor-build -o hello hello.hx");
    println!("    rayzor-build -O3 -o hello hello.hx");
    println!("    rayzor-build --target x86_64-unknown-linux-gnu -o hello hello.hx");
    println!("    rayzor-build --emit llvm-ir -o hello.ll hello.hx");
}
