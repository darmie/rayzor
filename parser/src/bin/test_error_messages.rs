use parser::incremental_parser_enhanced::parse_incrementally_enhanced;

fn main() {
    let input = std::fs::read_to_string("test_error_messages.hx").unwrap();
    let result = parse_incrementally_enhanced("test_error_messages.hx", &input);

    if result.has_errors() {
        println!("Errors found:\n{}", result.format_diagnostics(true));
    } else {
        println!("Parse succeeded");
    }
}
