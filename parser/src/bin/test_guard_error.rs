use parser::incremental_parser_enhanced::parse_incrementally_enhanced;

fn main() {
    let input = std::fs::read_to_string("test_guard_error.hx").unwrap();
    let result = parse_incrementally_enhanced("test_guard_error.hx", &input);

    if result.has_errors() {
        println!("Error found:");
        println!("{}", result.format_diagnostics(true));
    } else {
        println!("Parse succeeded");
    }
}
