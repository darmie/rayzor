use parser::parse_haxe_file;

#[test]
fn test_extractor_patterns() {
    let input = r#"
class ExtractorTest {
    public static function testExtractors(value:Dynamic):String {
        return switch (value) {
            // Method call extractor
            case _.toLowerCase() => "hello": "greeting";
            
            // Regex extractor
            case ~/^[a-z]+$/i.match(_) => true: "letters only";
            
            // Field access extractor
            case obj.field => "value": "field match";
            
            // Function call extractor
            case func() => result: "function result";
            
            // Simple variable extractor
            case x => y: "variable match";
            
            // Number extractor
            case 42 => answer: "the answer";
            
            default: "no match";
        };
    }
}
"#;

    match parse_haxe_file("test.hx", input, false) {
        Ok(file) => {
            assert!(!file.declarations.is_empty());
            println!("✓ All extractor patterns parsed successfully");
        }
        Err(e) => {
            panic!("Failed to parse extractor patterns: {}", e);
        }
    }
}
