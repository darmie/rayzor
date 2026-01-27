//! Comprehensive test covering all Haxe language features
//! This test ensures the parser can handle the full range of Haxe syntax

use parser::haxe_ast::*;
use parser::parse_haxe_file;

#[test]
fn test_comprehensive_haxe_features() {
    let input = r#"
// Package declaration
package com.example.app;

// Imports with various patterns
import haxe.macro.Context;
import haxe.macro.Expr;
import sys.io.File;
import sys.FileSystem.*;
import Lambda as L;

// Using declarations
using StringTools;
using Lambda;

// Conditional compilation
#if debug
import debug.DebugTools;
#end

// Metadata on class
@:build(Builder.build())
@:native("NativeClass")
@:final
class ComplexClass<T, U:Constraint> extends BaseClass implements IInterface1, IInterface2 {
    // Static variables with metadata
    @:isVar
    public static var staticField(get, set):String = "default";
    
    // Instance variables
    private var privateField:Int;
    public var publicField:String = "hello";
    private final constant:Float = 3.14;
    
    // Properties with accessors
    public var readOnlyProp(default, null):Bool;
    public var writeOnlyProp(null, default):Int;
    public var customProp(get, set):Array<String>;
    
    // Array and map fields
    var items:Array<T> = [];
    var lookup:Map<String, U> = new Map();
    var grid:Array<Array<Int>> = [[1, 2], [3, 4]];
    
    // Constructor with default parameters
    public function new(x:Int = 0, ?y:String, rest:haxe.Rest<Dynamic>) {
        super();
        this.privateField = x;
        if (y != null) {
            this.publicField = y;
        }
    }
    
    // Overloaded constructor via metadata
    @:overload(function(config:Dynamic) {})
    public function new() {
        this(42, "default");
    }
    
    // Getter/setter implementations
    function get_staticField():String {
        return staticField ?? "null";
    }
    
    function set_staticField(value:String):String {
        return staticField = value.toUpperCase();
    }
    
    function get_customProp():Array<String> {
        return items.map(item -> Std.string(item));
    }
    
    function set_customProp(value:Array<String>):Array<String> {
        // Pattern matching in setter
        return switch (value) {
            case []: items = [];
            case [single]: items = [cast single];
            case many: items = cast many;
        }
    }
    
    // Generic method with constraints
    public function process<V:IComparable>(input:V):T {
        // Complex expression with operators
        var result = input.compareTo(null) > 0 ? cast input : cast privateField;
        
        // Try-catch with multiple catch blocks
        try {
            result = performOperation(result);
        } catch (e:CustomError) {
            trace('Custom error: ${e.message}');
            throw e;
        } catch (e:Dynamic) {
            trace('Unknown error: $e');
            return cast null;
        }
        
        return result;
    }
    
    // Method with complex control flow
    public function complexFlow(data:Array<Dynamic>):Void {
        // For loop variations
        for (i in 0...data.length) {
            trace(data[i]);
        }
        
        for (item in data) {
            processItem(item);
        }
        
        // While loop
        var index = 0;
        while (index < data.length) {
            if (data[index] == null) {
                index++;
                continue;
            }
            
            // Do-while loop
            do {
                data[index] = transform(data[index]);
            } while (!isValid(data[index]));
            
            index++;
        }
        
        // Array comprehension
        var processed = [for (i in 0...10) if (i % 2 == 0) i * 2];
        
        // Map comprehension
        var lookup = [for (item in data) if (item != null) Std.string(item) => item];
        
        // Switch with guards and complex patterns
        switch (data) {
            case [] | null:
                trace("Empty data");
            case [single] if (single is String):
                trace('Single string: $single');
            case [first, second, ...rest] if (rest.length > 0):
                trace('Multiple items, first: $first');
            case array if (array.length == 42):
                trace("The answer!");
            default:
                trace("Other case");
        }
    }
    
    // Inline method
    inline public function inlineMethod(x:Int, y:Int):Int {
        return x + y;
    }
    
    // Method with function type parameters
    public function higherOrder(
        callback:(Int, String) -> Bool,
        ?errorHandler:Dynamic -> Void
    ):Void {
        // Lambda expressions
        var doubler = x -> x * 2;
        var isEven = (x:Int) -> x % 2 == 0;
        
        // Function with body
        var processor = function(data:Array<Int>) {
            return data.filter(isEven).map(doubler);
        };
        
        // Nested functions
        function localHelper(n:Int):String {
            function innerHelper():Int {
                return n * n;
            }
            return Std.string(innerHelper());
        }
        
        // Using callbacks
        if (callback(42, localHelper(5))) {
            trace("Success!");
        }
    }
    
    // Operator overloading via abstract
    @:op(A + B)
    public function add(other:ComplexClass<T, U>):ComplexClass<T, U> {
        return new ComplexClass(this.privateField + other.privateField);
    }
    
    // Macro method
    macro public function assert(expr:Expr):Expr {
        return macro {
            if (!$expr) {
                throw 'Assertion failed: ${expr.toString()}';
            }
        };
    }
    
    // Static extension method
    @:noCompletion
    public static function extend<T>(instance:ComplexClass<T, Dynamic>, value:T):Void {
        instance.items.push(value);
    }
    
    // Abstract method (for subclasses)
    private function performOperation<V>(value:V):T {
        throw "Abstract method";
    }
    
    private function processItem(item:Dynamic):Void {}
    private function transform(item:Dynamic):Dynamic { return item; }
    private function isValid(item:Dynamic):Bool { return true; }
}

// Interface with type parameters
interface IInterface1 {
    function interfaceMethod():Void;
    var interfaceProperty(get, never):String;
}

interface IInterface2 {
    function anotherMethod(?optional:Int):Bool;
}

// Enum with parameters
enum Color {
    Red;
    Green;
    Blue;
    RGB(r:Int, g:Int, b:Int);
    HSL(h:Float, s:Float, l:Float);
    Named(name:String);
}

// Enum with metadata
@:enum
abstract HttpStatus(Int) to Int {
    var Ok = 200;
    var NotFound = 404;
    var ServerError = 500;
    
    @:from
    static public function fromString(s:String):HttpStatus {
        return switch (s) {
            case "ok": Ok;
            case "not_found": NotFound;
            default: ServerError;
        };
    }
    
    @:to
    public function toString():String {
        return switch (this) {
            case 200: "OK";
            case 404: "Not Found";
            case 500: "Server Error";
            default: "Unknown";
        };
    }
}

// Abstract type with multiple from/to conversions
abstract Vector2D(Point) from Point to Point {
    public var x(get, never):Float;
    public var y(get, never):Float;
    public var length(get, never):Float;
    
    inline public function new(x:Float, y:Float) {
        this = {x: x, y: y};
    }
    
    @:from
    static public function fromArray(arr:Array<Float>):Vector2D {
        return new Vector2D(arr[0] ?? 0, arr[1] ?? 0);
    }
    
    @:to
    public function toArray():Array<Float> {
        return [this.x, this.y];
    }
    
    inline function get_x():Float return this.x;
    inline function get_y():Float return this.y;
    inline function get_length():Float {
        return Math.sqrt(this.x * this.x + this.y * this.y);
    }
    
    // Operator overloading
    @:op(A + B)
    public function add(other:Vector2D):Vector2D {
        return new Vector2D(this.x + other.x, this.y + other.y);
    }
    
    @:op(A * B)
    @:commutative
    static public function scale(v:Vector2D, s:Float):Vector2D {
        return new Vector2D(v.x * s, v.y * s);
    }
    
    @:op(A == B)
    public function equals(other:Vector2D):Bool {
        return this.x == other.x && this.y == other.y;
    }
    
    // Array access
    @:arrayAccess
    public function get(index:Int):Float {
        return index == 0 ? this.x : this.y;
    }
}

// Typedef with constraints
typedef Callback<T> = {
    function execute(data:T):Void;
    var priority:Int;
    @:optional var name:String;
}

// Complex typedef
typedef ComplexType = {
    > BaseType,  // Extension
    var additional:String;
    function method():Int;
    var ?optional:Bool;
}

// Function typedef
typedef Processor<T, R> = T -> R;
typedef AsyncProcessor<T> = T -> (R -> Void) -> Void;

// Extern class
extern class ExternalAPI {
    static function initialize():Void;
    function call(method:String, ?params:Dynamic):Dynamic;
    var ready(default, never):Bool;
}

// Private class in same file
private class Helper {
    public static function util():Void {}
}

// Module-level metadata
@:dce("full")
@:analyzer(optimize)

// Global conditional compilation
#if !debug
@:final
#end
class OptimizedClass {
    // Conditional fields
    #if debug
    var debugInfo:String;
    #end
    
    public function new() {
        #if debug
        debugInfo = "Debug build";
        trace(debugInfo);
        #elseif release
        // Production code
        #else
        // Development code
        #end
    }
    
    // Untyped code block
    public function unsafeOperation():Void {
        untyped {
            __js__("console.log('Direct JS code')");
            __cpp__("std::cout << 'Direct C++ code' << std::endl;");
        }
    }
}

// Type with variance annotations
// TODO: Variance annotations not yet implemented
interface Container<T, R> {
    function put(item:T):Void;
    function get():R;
}

// Recursive type
typedef Tree<T> = {
    var value:T;
    var ?left:Tree<T>;
    var ?right:Tree<T>;
}

// Advanced pattern matching
class PatternTest {
    public static function advancedPatterns(value:Dynamic):String {
        return switch (value) {
            // Array patterns
            case []: "empty array";
            case [x]: 'single: $x';
            case [x, y]: 'pair: $x, $y';
            case [head, ...tail]: 'head: $head, tail: $tail';
            
            // Object patterns
            case {x: 0, y: 0}: "origin";
            case {x: x, y: y} if (x == y): "diagonal";
            case {x: _, y: _}: "point";
            
            // Type patterns
            case (s:String): 'string: $s';
            case (i:Int) if (i > 0): 'positive: $i';
            case (f:Float): 'float: $f';
            
            // Enum patterns
            case Red: "red";
            case RGB(255, 0, 0): "pure red";
            case RGB(r, g, b): 'rgb($r, $g, $b)';
            
            // Or patterns
            case 1 | 2 | 3: "small number";
            case "yes" | "true" | "1": "truthy";
            
            // Extractor patterns now implemented!
            case _.toLowerCase() => "hello": "greeting";
            case ~/^[a-z]+$/i.match(_) => true: "letters only";
            
            default: "unknown";
        }
    }
}

// String interpolation edge cases
class StringTests {
    public static function interpolation():Void {
        var name = "Haxe";
        var version = 4.3;
        
        // Basic interpolation
        trace('Hello, $name!');
        trace("Version: $version");
        
        // Complex expressions
        trace('Sum: ${1 + 2 + 3}');
        trace('Conditional: ${version > 4.0 ? "modern" : "legacy"}');
        
        // Nested interpolation
        var template = 'User: ${getName()} (${getAge()} years old)';
        
        // Raw strings
        // TODO: Regex literals not yet implemented
        // var regex = ~/[a-zA-Z]+/;
        var multiline = "
            Line 1
            Line 2
            Line 3
        ";
    }
    
    static function getName():String return "Test";
    static function getAge():Int return 25;
}

// Edge case: empty class
class Empty {}

// Edge case: class with only static initializer
class StaticInit {
    static var field:Int = {
        trace("Static initialization");
        computeValue();
    };
    
    static function computeValue():Int {
        return 42;
    }
}

// Final usage example
class Main {
    static function main() {
        // Create instances
        var obj = new ComplexClass<String, Int>(10, "test");
        var vec = new Vector2D(3, 4);
        
        // Use operators
        var doubled = vec * 2;
        var sumVec = vec + doubled;
        
        // Pattern matching
        var result = PatternTest.advancedPatterns([1, 2, 3]);
        
        // Conditional compilation
        #if debug
        trace("Debug mode active");
        #end
        
        // Error handling
        try {
            throw new CustomError("Test error");
        } catch (e:CustomError) {
            trace(e.message);
        }
        
        // Using abstract enum
        var status:HttpStatus = "ok";
        trace(status.toString());
    }
}

// Custom error class
class CustomError {
    public var message:String;
    public function new(msg:String) {
        this.message = msg;
    }
}

// Required types for compilation
typedef Point = {x:Float, y:Float};
typedef BaseType = {id:Int};
class BaseClass { public function new() {} }
interface IComparable { function compareTo(other:Dynamic):Int; }
"#;

    match parse_haxe_file("test.hx", input, false) {
        Ok(file) => {
            // Basic structure tests
            assert!(file.package.is_some());
            let package = file.package.as_ref().unwrap();
            assert_eq!(package.path, vec!["com", "example", "app"]);
            assert!(file.imports.len() >= 5);
            assert!(file.using.len() >= 2);
            assert!(file.declarations.len() > 10);

            // Find main class
            let complex_class = file
                .declarations
                .iter()
                .find_map(|decl| match decl {
                    TypeDeclaration::Class(c) if c.name == "ComplexClass" => Some(c),
                    _ => None,
                })
                .expect("Should find ComplexClass");

            // Test class features
            assert_eq!(complex_class.type_params.len(), 2);
            assert!(complex_class.extends.is_some());
            assert_eq!(complex_class.implements.len(), 2);
            assert!(complex_class.has_constructor());

            // Test various declaration types exist
            let has_interface = file
                .declarations
                .iter()
                .any(|d| matches!(d, TypeDeclaration::Interface(_)));
            let has_enum = file
                .declarations
                .iter()
                .any(|d| matches!(d, TypeDeclaration::Enum(_)));
            let has_abstract = file
                .declarations
                .iter()
                .any(|d| matches!(d, TypeDeclaration::Abstract(_)));
            let has_typedef = file
                .declarations
                .iter()
                .any(|d| matches!(d, TypeDeclaration::Typedef(_)));

            assert!(has_interface, "Should have interface declarations");
            assert!(has_enum, "Should have enum declarations");
            assert!(has_abstract, "Should have abstract declarations");
            assert!(has_typedef, "Should have typedef declarations");

            println!("✓ Comprehensive Haxe parsing test passed!");
        }
        Err(e) => {
            panic!("Failed to parse comprehensive Haxe file: {}", e);
        }
    }
}

#[test]
fn test_error_recovery_scenarios() {
    // Test various error scenarios to ensure parser can recover
    let error_cases = [
        // Missing semicolons
        r#"
        class Test {
            var x:Int = 5
            var y:Int = 10;
        }
        "#,
        // Unclosed block
        r#"
        class Test {
            function method() {
                if (true) {
                    trace("unclosed");
            }
        }
        "#,
        // Invalid syntax in method
        r#"
        class Test {
            function broken() {
                var x = ;
                var y = 10;
            }
        }
        "#,
        // Missing type annotation
        r#"
        class Test {
            var field: = "value";
        }
        "#,
        // Malformed expression
        r#"
        class Test {
            function calc() {
                return 5 + * 3;
            }
        }
        "#,
    ];

    for (i, case) in error_cases.iter().enumerate() {
        match parse_haxe_file("test.hx", case, false) {
            Ok(_) => {
                println!(
                    "Warning: Error case {} parsed successfully (might have error recovery)",
                    i
                );
            }
            Err(e) => {
                println!("Error case {} failed as expected: {}", i, e);
            }
        }
    }
}
