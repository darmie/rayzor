use parser::haxe_parser::parse_haxe_file;
use std::fs;

fn main() {
    // Test 1: Original test case
    let content1 = r#"extern class MathTest {
	#if (flash || cpp || eval)
	static function ffloor(v:Float):Float;
	#else
	static inline function ffloor(v:Float):Float {
		return 1.0;
	}
	#end
}
"#;

    println!("=== Test 1: Block-level preprocessor with inline function ===");
    println!("Content:\n{}", content1);
    println!("\nParsing...");

    match parse_haxe_file("test.hx", content1, false) {
        Ok(file) => {
            println!("✅ SUCCESS! Parsed {} declarations", file.declarations.len());
        },
        Err(e) => {
            println!("❌ FAILED to parse:");
            println!("{:?}", e);
            return;
        }
    }

    // Test 2: Math.hx file
    println!("\n\n=== Test 2: Full Math.hx file ===");
    match fs::read_to_string("compiler/haxe-std/Math.hx") {
        Ok(content2) => {
            println!("Parsing Math.hx ({} bytes)...", content2.len());
            match parse_haxe_file("Math.hx", &content2, false) {
                Ok(file) => {
                    println!("✅ SUCCESS! Parsed {} declarations from Math.hx", file.declarations.len());
                },
                Err(e) => {
                    println!("❌ FAILED to parse Math.hx:");
                    println!("{:?}", e);
                }
            }
        },
        Err(e) => {
            println!("❌ Failed to read Math.hx: {}", e);
        }
    }
}
