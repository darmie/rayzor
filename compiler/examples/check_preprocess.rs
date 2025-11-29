//! Check preprocessor output for Bytes.hx

use parser::preprocessor::{preprocess, PreprocessorConfig};

fn main() {
    let source = std::fs::read_to_string("compiler/haxe-std/haxe/io/Bytes.hx").expect("Failed to read Bytes.hx");
    
    // Test with rayzor define
    let mut config = PreprocessorConfig::default();
    config.defines.insert("rayzor".to_string());
    let result = preprocess(&source, &config);
    
    println!("=== Preprocessed output (first 500 chars) ===");
    println!("{}", &result[..result.len().min(500)]);
    
    // Show the non-comment lines
    println!("\n=== Non-comment lines ===");
    for (i, line) in result.lines().enumerate().take(40) {
        let trimmed = line.trim();
        if !trimmed.is_empty() 
            && !trimmed.starts_with("/*") 
            && !trimmed.starts_with("*") 
            && !trimmed.starts_with("//") 
        {
            println!("{:3}: {}", i + 1, line);
        }
    }
    
    // Check if it contains typedef or class
    if result.contains("typedef Bytes") {
        println!("\n✅ Found 'typedef Bytes' - preprocessor correctly chose rayzor branch");
    } else if result.contains("class Bytes") {
        println!("\n❌ Found 'class Bytes' - preprocessor incorrectly chose non-rayzor branch");
    } else {
        println!("\n⚠️ Neither typedef nor class found");
    }
}
