#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    unreachable_patterns,
    unused_mut,
    unused_assignments,
    unused_parens
)]
#![allow(
    clippy::single_component_path_imports,
    clippy::for_kv_map,
    clippy::explicit_auto_deref
)]
#![allow(
    clippy::println_empty_string,
    clippy::len_zero,
    clippy::useless_vec,
    clippy::field_reassign_with_default
)]
#![allow(
    clippy::needless_borrow,
    clippy::redundant_closure,
    clippy::bool_assert_comparison
)]
#![allow(
    clippy::empty_line_after_doc_comments,
    clippy::useless_format,
    clippy::clone_on_copy
)]
//! Test if Math.hx can be parsed

use std::fs;

fn main() {
    println!("Testing Math.hx parsing...\n");

    let math_path = "/Users/amaterasu/Vibranium/rayzor/compiler/haxe-std/Math.hx";

    match fs::read_to_string(math_path) {
        Ok(source) => {
            println!("✓ Read Math.hx ({} bytes)", source.len());

            // Try to parse it
            use parser::haxe_parser::haxe_file;

            match haxe_file("Math.hx", &source, &source) {
                Ok((_remaining, ast)) => {
                    println!("✓ Successfully parsed Math.hx!");
                    println!("  Package: {:?}", ast.package);
                    println!("  Imports: {}", ast.imports.len());
                    println!("  Declarations: {}", ast.declarations.len());
                }
                Err(e) => {
                    println!("❌ Failed to parse Math.hx:");
                    println!("  Error: {}", e);
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to read Math.hx: {}", e);
        }
    }
}
