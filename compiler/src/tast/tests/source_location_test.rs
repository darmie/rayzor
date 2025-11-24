#[cfg(test)]
mod source_location_tests {
    use crate::pipeline::compile_haxe_source;

    #[test]
    fn test_source_location_accuracy() {
        let haxe_code = r#"// Line 1
// Line 2
class Test {  // Line 3
    // Line 4
    public function new() {  // Line 5
        // Line 6
    }  // Line 7
    // Line 8
    public function method():Void {  // Line 9
        // Line 10
    }  // Line 11
}  // Line 12
// Line 13
class Child extends Test {  // Line 14
    // Line 15
    public function method():Void {  // Line 16 - should show error here
        // Line 17
    }  // Line 18
}  // Line 19"#;
        
        let result = compile_haxe_source(haxe_code);
        
        println!("\n=== Source Location Test ===\n");
        println!("Source code has {} lines", haxe_code.lines().count());
        
        for (i, error) in result.errors.iter().enumerate() {
            println!("\nError {}:", i + 1);
            println!("Location: line {}, column {}", error.location.line, error.location.column);
            println!("Message: {}", error.message);
            
            // Check if the error is on the expected line
            if error.message.contains("method") && error.message.contains("override") {
                // The error should be around line 16 where the child method is defined
                println!("Expected error around line 16, got line {}", error.location.line);
            }
        }
        
        // Also print the raw diagnostic to see what the diagnostic system shows
        println!("\n--- Raw Diagnostic Output ---");
        for error in &result.errors {
            if error.message.contains("override") {
                // Extract just the location line from the diagnostic
                if let Some(location_line) = error.message.lines().find(|l| l.contains("-->")) {
                    println!("Diagnostic location: {}", location_line);
                }
            }
        }
    }
    
    #[test]
    fn test_span_conversion_simple() {
        // Direct test of span conversion
        use source_map::SourceMap;
        use parser::Span;
        
        let code = "line 1\nline 2\nline 3";
        let mut source_map = SourceMap::new();
        let file_id = source_map.add_file("test.hx".to_string(), code.to_string());
        
        // Create a span converter
        let converter = crate::tast::span_conversion::SpanConverter::new(source_map, file_id);
        
        // Test converting a span that starts at "line 2" (offset 7)
        let span = Span::new(7, 13); // "line 2"
        let location = converter.convert_span(span);
        
        println!("\nTest span conversion:");
        println!("Code: {:?}", code);
        println!("Span: {:?} (bytes {} to {})", span, span.start, span.end);
        println!("Converted location: line {}, column {}", location.line, location.column);
        
        // The span starting at byte 7 should be line 2, column 1
        assert_eq!(location.line, 2, "Expected line 2");
        assert_eq!(location.column, 1, "Expected column 1");
    }
    
    #[test]
    fn test_debug_parser_spans() {
        // Parse our test code and look at parser spans
        let haxe_code = r#"class Test {
    public function method():Void {
    }
}
class Child extends Test {
    public function method():Void {
    }
}"#;
        
        let parse_result = parser::parse_haxe_file_with_diagnostics("test.hx", haxe_code).unwrap();
        let ast = parse_result.file;
        
        println!("\n=== Debug Parser Spans ===");
        
        // Find the Child class method
        for type_decl in &ast.declarations {
            if let parser::TypeDeclaration::Class(class_decl) = type_decl {
                if class_decl.name == "Child" {
                    println!("\nFound Child class");
                    for field in &class_decl.fields {
                        if let parser::ClassFieldKind::Function(func) = &field.kind {
                            if func.name == "method" {
                                println!("  Found method in Child class");
                                println!("  Field span: {:?}", field.span);
                                println!("  Function span: {:?}", func.span);
                                
                                // Calculate expected line manually
                                let mut line = 1;
                                let mut col = 1;
                                for (i, ch) in haxe_code.chars().enumerate() {
                                    if i == field.span.start {
                                        println!("  Field starts at line {}, column {} (byte {})", line, col, i);
                                        break;
                                    }
                                    if ch == '\n' {
                                        line += 1;
                                        col = 1;
                                    } else {
                                        col += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}