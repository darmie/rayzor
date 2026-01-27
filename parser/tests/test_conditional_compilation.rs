//! Test conditional compilation parsing

use parser::parse_haxe_file;

#[test]
fn test_simple_conditional() {
    let input = r#"
#if debug
class DebugTools {
    public function new() {}
}
#end
"#;

    match parse_haxe_file("test.hx", input, false) {
        Ok(ast) => println!("✓ Simple #if/#end works: {:?}", ast),
        Err(e) => println!("✗ Simple #if/#end failed: {}", e),
    }
}

#[test]
fn test_if_else_conditional() {
    let input = r#"
#if debug
class DebugTools {
    public function new() {}
}
#else
class ProductionTools {
    public function new() {}
}
#end
"#;

    match parse_haxe_file("test.hx", input, false) {
        Ok(ast) => println!("✓ #if/#else/#end works: {:?}", ast),
        Err(e) => println!("✗ #if/#else/#end failed: {}", e),
    }
}

#[test]
fn test_elseif_conditional() {
    let input = r#"
#if debug
class DebugTools {}
#elseif test
class TestTools {}
#else
class ProductionTools {}
#end
"#;

    match parse_haxe_file("test.hx", input, false) {
        Ok(ast) => println!("✓ #if/#elseif/#else/#end works: {:?}", ast),
        Err(e) => println!("✗ #if/#elseif/#else/#end failed: {}", e),
    }
}

#[test]
fn test_conditional_in_class() {
    let input = r#"
class MyClass {
    #if debug
    public var debugFlag:Bool = true;
    #end
    
    public function new() {
        #if debug
        trace("Debug mode");
        #else
        trace("Release mode");
        #end
    }
}
"#;

    match parse_haxe_file("test.hx", input, false) {
        Ok(ast) => println!("✓ Conditional in class works: {:?}", ast),
        Err(e) => println!("✗ Conditional in class failed: {}", e),
    }
}

#[test]
fn test_complex_conditions() {
    let input = r#"
#if (debug && !release)
class DebugOnly {}
#end

#if (js || nodejs)
class JsPlatform {}
#end

#if (!cpp && !cs && !java)
class DynamicPlatform {}
#end
"#;

    match parse_haxe_file("test.hx", input, false) {
        Ok(ast) => println!("✓ Complex conditions work: {:?}", ast),
        Err(e) => println!("✗ Complex conditions failed: {}", e),
    }
}

#[test]
fn test_nested_conditionals() {
    let input = r#"
#if debug
    #if js
    class DebugJs {}
    #else
    class DebugOther {}
    #end
#end
"#;

    match parse_haxe_file("test.hx", input, false) {
        Ok(ast) => println!("✓ Nested conditionals work: {:?}", ast),
        Err(e) => println!("✗ Nested conditionals failed: {}", e),
    }
}

#[test]
fn test_conditional_with_imports() {
    let input = r#"
package test;

#if nodejs
import js.node.Fs;
#else
import sys.FileSystem;
#end

class FileHandler {
    public function new() {}
}
"#;

    match parse_haxe_file("test.hx", input, false) {
        Ok(ast) => println!("✓ Conditional imports work: {:?}", ast),
        Err(e) => println!("✗ Conditional imports failed: {}", e),
    }
}
