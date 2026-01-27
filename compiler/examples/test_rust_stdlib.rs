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

// NOTE: This test is currently disabled because the rayzor_runtime string functions
// (haxe_string_from_bytes, haxe_string_concat, haxe_string_len, haxe_string_free)
// are now private. This test needs to be rewritten to use the public API.

fn main() {
    println!("=== Rust Stdlib Integration Test ===");
    println!("NOTE: This test is currently disabled because runtime string functions are private.");
    println!("The functions haxe_string_from_bytes, haxe_string_concat, haxe_string_len,");
    println!("and haxe_string_free are no longer public in rayzor_runtime::string.");
}
