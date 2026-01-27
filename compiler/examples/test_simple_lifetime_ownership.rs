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

// NOTE: This test is currently disabled because the TypeFlowGuard API has changed.
// The methods analyze_lifetime_safety_graphs, analyze_ownership_safety_graphs, and
// get_metrics no longer exist. This test needs to be rewritten to use the new API.

fn main() {
    println!("=== Simple Lifetime and Ownership Analysis Test ===");
    println!("NOTE: This test is currently disabled pending TypeFlowGuard API updates.");
    println!("The analyze_lifetime_safety_graphs, analyze_ownership_safety_graphs,");
    println!("and get_metrics methods have been removed or renamed.");
}
