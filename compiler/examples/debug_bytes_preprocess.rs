//! Debug preprocessor output for Bytes.hx

use parser::preprocessor::{preprocess, PreprocessorConfig};

fn main() {
    let source = std::fs::read_to_string("compiler/haxe-std/haxe/io/Bytes.hx").expect("Failed to read Bytes.hx");
    
    // Test with rayzor define
    let config = PreprocessorConfig::default();
    let result = preprocess(&source, &config);
    
    // Print all non-empty lines
    println!("=== All non-empty lines from preprocessed Bytes.hx ===");
    for (i, line) in result.lines().enumerate() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            println!("{:4}: {}", i + 1, line);
        }
    }
    
    // Count lines
    let total_lines = result.lines().count();
    let non_empty = result.lines().filter(|l| !l.trim().is_empty()).count();
    println!("\nTotal lines: {}, Non-empty: {}", total_lines, non_empty);
}
