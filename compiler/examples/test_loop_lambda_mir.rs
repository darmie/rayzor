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

// NOTE: This test is currently disabled because it uses cranelift_jit_backend::CraneliftBackend
// which is not a valid crate dependency, and compiler::ir::builder::MirBuilder which has moved
// to compiler::ir::mir_builder::MirBuilder. This test needs to be rewritten.

fn main() {
    println!("=== Loop + Lambda MIR Test ===");
    println!("NOTE: This test is currently disabled pending API updates.");
    println!("The cranelift_jit_backend crate is not available and MirBuilder has moved.");
}
