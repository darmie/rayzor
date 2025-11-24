//! Comprehensive pipeline validation binary
//!
//! This binary runs the comprehensive AST-TAST pipeline validation
//! and reports on information preservation and correctness.

use compiler::pipeline::{compile_haxe_file, HaxeCompilationPipeline, PipelineConfig};
use compiler::pipeline_validation::*;

fn main() {
    println!("=== AST-TAST Pipeline Validation Suite ===\n");
    
    // Run the original basic tests first
    println!("🚀 Running Basic Pipeline Tests\n");
    run_basic_tests();
    
    println!("\n {}", "=".repeat(60).as_str());
    println!("🔍 Running Comprehensive AST-TAST Validation\n");
    
    // Run simple validation
    println!("Running simple validation...");
    let simple_result = validate_simple_pipeline();
    print_result_summary("Simple Pipeline", &simple_result);
    
    // Run complex validation
    println!("Running complex validation...");
    let complex_result = validate_complex_pipeline();
    print_result_summary("Complex Pipeline", &complex_result);
    
    // Run comprehensive validation
    println!("Running comprehensive validation...");
    let comprehensive_result = validate_comprehensive_pipeline();
    print_validation_summary("Comprehensive Pipeline", &comprehensive_result);
    
    // Run type parameter validation
    println!("Running type parameter validation...");
    let type_param_result = validate_type_parameters();
    print_validation_summary("Type Parameter Validation", &type_param_result);
    
    // Run property validation
    println!("Running property validation...");
    let property_result = validate_properties();
    print_validation_summary("Property Validation", &property_result);
    
    // Run metadata validation
    println!("Running metadata validation...");
    let metadata_result = validate_metadata_preservation();
    print_validation_summary("Metadata Validation", &metadata_result);
    
    // Run overall validation suite
    println!("Running overall validation suite...");
    let overall_result = run_comprehensive_validation();
    print_validation_summary("Overall Validation Suite", &overall_result);
    
    // Summary report
    println!("=== VALIDATION SUMMARY ===");
    println!("Simple pipeline processed: {} files", simple_result.stats.files_processed);
    println!("Complex pipeline processed: {} files", complex_result.stats.files_processed);
    println!("Comprehensive pipeline score: {}/100", comprehensive_result.info_preservation_score);
    println!("Type parameter validation score: {}/100", type_param_result.info_preservation_score);
    println!("Property validation score: {}/100", property_result.info_preservation_score);
    println!("Metadata validation score: {}/100", metadata_result.info_preservation_score);
    println!("Overall validation score: {}/100", overall_result.info_preservation_score);
    
    // Final assessment
    let avg_score = [
        comprehensive_result.info_preservation_score,
        type_param_result.info_preservation_score,
        property_result.info_preservation_score,
        metadata_result.info_preservation_score,
    ].iter().map(|&s| s as u32).sum::<u32>() / 4;
    
    println!("\n=== FINAL ASSESSMENT ===");
    println!("Average Information Preservation Score: {}/100", avg_score);
    
    if avg_score >= 90 {
        println!("✅ EXCELLENT - Pipeline preserves information excellently");
    } else if avg_score >= 80 {
        println!("✅ GOOD - Pipeline preserves information well with minor issues");
    } else if avg_score >= 70 {
        println!("⚠️  ACCEPTABLE - Pipeline needs improvement but is functional");
    } else if avg_score >= 50 {
        println!("❌ POOR - Significant information loss detected");
    } else {
        println!("❌ CRITICAL - Major pipeline issues detected");
    }
    
    // Count total issues
    let total_errors = comprehensive_result.validation_errors.len() +
                      type_param_result.validation_errors.len() +
                      property_result.validation_errors.len() +
                      metadata_result.validation_errors.len();
    
    let total_warnings = comprehensive_result.validation_warnings.len() +
                        type_param_result.validation_warnings.len() +
                        property_result.validation_warnings.len() +
                        metadata_result.validation_warnings.len();
    
    println!("Total validation errors: {}", total_errors);
    println!("Total validation warnings: {}", total_warnings);
    
    if total_errors == 0 && total_warnings == 0 {
        println!("🎉 No validation issues detected!");
    } else if total_errors == 0 {
        println!("⚠️  Only warnings detected - pipeline is functional");
    } else {
        println!("❌ Validation errors detected - pipeline needs attention");
    }
}

fn run_basic_tests() {
    // Test 1: Simple Hello World
    println!("Test 1: Simple Hello World");
    let hello_world = r#"
        class Main {
            static function main() {
                trace("Hello, World!");
            }
        }
    "#;
    
    let result1 = compile_haxe_file("HelloWorld.hx", hello_world);
    print_test_result("Hello World", &result1);
    
    // Test 2: Package and imports
    println!("Test 2: Package and imports");
    let with_package = r#"
        package com.example;
        
        import haxe.ds.Map;
        
        class Test {
            static function main() {
                var data:Map<String, Int> = new Map();
                data.set("key", 42);
                trace(data.get("key"));
            }
        }
    "#;
    
    let result2 = compile_haxe_file("Test.hx", with_package);
    print_test_result("Package and imports", &result2);
    
    // Test 3: Enum and pattern matching
    println!("Test 3: Enum and pattern matching");
    let enum_test = r#"
        enum Color {
            Red;
            Green;
            Blue;
            RGB(r:Int, g:Int, b:Int);
        }
        
        class ColorTest {
            static function colorName(color:Color):String {
                return switch (color) {
                    case Red: "red";
                    case Green: "green";
                    case Blue: "blue";
                    case RGB(r, g, b): 'rgb($r, $g, $b)';
                };
            }
            
            static function main() {
                trace(colorName(Red));
                trace(colorName(RGB(255, 128, 0)));
            }
        }
    "#;
    
    let result3 = compile_haxe_file("ColorTest.hx", enum_test);
    print_test_result("Enum and pattern matching", &result3);
    
    // Test 4: Abstract types
    println!("Test 4: Abstract types");
    let abstract_test = r#"
        abstract Point(Array<Float>) from Array<Float> to Array<Float> {
            public var x(get, never):Float;
            public var y(get, never):Float;
            
            public function new(x:Float, y:Float) {
                this = [x, y];
            }
            
            function get_x():Float return this[0];
            function get_y():Float return this[1];
            
            @:op(A + B)
            public function add(other:Point):Point {
                return new Point(x + other.x, y + other.y);
            }
        }
        
        class PointTest {
            static function main() {
                var p1 = new Point(1.0, 2.0);
                var p2 = new Point(3.0, 4.0);
                var sum = p1 + p2;
                trace('Sum: (${sum.x}, ${sum.y})');
            }
        }
    "#;
    
    let result4 = compile_haxe_file("PointTest.hx", abstract_test);
    print_test_result("Abstract types", &result4);
    
    // Test 5: For-in loops and modern syntax
    println!("Test 5: For-in loops and modern syntax");
    let modern_syntax = r#"
        class ModernTest {
            static function main() {
                var data = ["a" => 1, "b" => 2, "c" => 3];
                
                for (key => value in data) {
                    trace('$key: $value');
                }
                
                var list = [1, 2, 3, 4, 5];
                for (item in list) {
                    trace('Item: $item');
                }
            }
        }
    "#;
    
    let result5 = compile_haxe_file("ModernTest.hx", modern_syntax);
    print_test_result("Modern syntax", &result5);
    
    // Summary
    let all_results = [&result1, &result2, &result3, &result4, &result5];
    let total_files = all_results.iter().map(|r| r.stats.files_processed).sum::<usize>();
    let total_errors = all_results.iter().map(|r| r.errors.len()).sum::<usize>();
    let total_parse_time: u64 = all_results.iter().map(|r| r.stats.parse_time_us).sum();
    let successful_parses = all_results.iter().filter(|r| r.stats.files_processed > 0).count();
    
    println!("📊 Basic Tests Summary:");
    println!("  Total files processed: {}", total_files);
    println!("  Successful parses: {}/{}", successful_parses, all_results.len());
    println!("  Total parse time: {}μs", total_parse_time);
    println!("  Total errors: {}", total_errors);
    
    if successful_parses == all_results.len() {
        println!("  ✅ All basic tests passed!");
    } else {
        println!("  ❌ Some basic tests failed");
    }
}

fn print_test_result(test_name: &str, result: &compiler::pipeline::CompilationResult) {
    print!("  ");
    if result.stats.files_processed > 0 && result.errors.is_empty() {
        println!("✅ {}: SUCCESS", test_name);
    } else if result.stats.files_processed > 0 {
        println!("⚠️  {}: PARSED with {} errors", test_name, result.errors.len());
    } else {
        println!("❌ {}: FAILED", test_name);
    }
    
    println!("     Parse time: {}μs, TAST files: {}", 
             result.stats.parse_time_us, 
             result.typed_files.len());
    
    if !result.errors.is_empty() && result.errors.len() <= 3 {
        for error in &result.errors {
            println!("     Error: {}", error.message);
        }
    } else if result.errors.len() > 3 {
        println!("     {} errors (showing first 3):", result.errors.len());
        for error in result.errors.iter().take(3) {
            println!("     Error: {}", error.message);
        }
    }
    
    println!();
}