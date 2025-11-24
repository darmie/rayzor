//! Standard Library Loader
//! 
//! This module handles loading Haxe standard library types from source files
//! rather than hardcoding them. This follows Haxe's actual implementation.

use std::path::{Path, PathBuf};
use std::collections::HashMap;
use parser::{parse_haxe_file_with_diagnostics, HaxeFile, ErrorFormatter};

/// Strip ANSI escape codes from a string for cleaner error output
fn strip_ansi_codes(input: &str) -> String {
    // Simple regex to remove ANSI escape codes
    // This matches ESC[ followed by any number of digits, semicolons, and ends with a letter
    let mut result = String::new();
    let mut chars = input.chars().peekable();
    
    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            // Skip the ESC[
            chars.next(); // consume '['
            
            // Skip until we find a letter (the final character of the escape sequence)
            while let Some(c) = chars.next() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            result.push(ch);
        }
    }
    
    result
}

/// Standard library loader configuration
#[derive(Debug, Clone)]
pub struct StdLibConfig {
    /// Paths to search for standard library files
    pub std_paths: Vec<PathBuf>,
    
    /// Whether to load import.hx files automatically
    pub load_import_hx: bool,
    
    /// Package imports that are always available (top-level)
    pub default_imports: Vec<String>,
}

impl Default for StdLibConfig {
    fn default() -> Self {
        Self {
            std_paths: vec![
                // Common Haxe standard library locations
                PathBuf::from("/usr/lib/haxe/std"),
                PathBuf::from("/usr/local/lib/haxe/std"),
                PathBuf::from("~/.haxe/std"),
            ],
            load_import_hx: true,
            default_imports: vec![
                // Top-level types that are always imported
                "StdTypes.hx".to_string(),  // Contains Int, Float, String, Bool, etc.
            ],
        }
    }
}

/// Loader for Haxe standard library
pub struct StdLibLoader {
    config: StdLibConfig,
    /// Cache of loaded files to avoid re-parsing
    loaded_files: HashMap<PathBuf, HaxeFile>,
}

impl StdLibLoader {
    pub fn new(config: StdLibConfig) -> Self {
        Self {
            config,
            loaded_files: HashMap::new(),
        }
    }
    
    /// Load a standard library file by name
    pub fn load_std_file(&mut self, filename: &str) -> Result<&HaxeFile, String> {
        // Try to find the file in standard paths
        for std_path in &self.config.std_paths {
            let file_path = std_path.join(filename);
            if file_path.exists() {
                return self.load_file(&file_path);
            }
        }
        
        Err(format!("Standard library file '{}' not found in any std path", filename))
    }
    
    /// Load a specific file
    fn load_file(&mut self, path: &Path) -> Result<&HaxeFile, String> {
        // Check cache first
        if self.loaded_files.contains_key(path) {
            return Ok(self.loaded_files.get(path).unwrap());
        }
        
        // Read and parse the file
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
            
        let parse_result = parse_haxe_file_with_diagnostics(
            path.to_str().unwrap_or("unknown.hx"),
            &content
        ).map_err(|e| {
            // Strip ANSI color codes from error message for cleaner output
            let clean_error = strip_ansi_codes(&e);
            format!("Failed to parse {}: {}", path.display(), clean_error)
        })?;
        
        let haxe_file = parse_result.file;
        
        // Cache the parsed file
        self.loaded_files.insert(path.to_path_buf(), haxe_file);
        Ok(self.loaded_files.get(path).unwrap())
    }
    
    /// Load all default imports (top-level types)
    pub fn load_default_imports(&mut self) -> Vec<HaxeFile> {
        let mut files = Vec::new();
        
        let default_imports = self.config.default_imports.clone();
        for import_file in &default_imports {
            match self.load_std_file(import_file) {
                Ok(file) => files.push(file.clone()),
                Err(e) => {
                    // Log warning but continue - some files might not exist
                    eprintln!("Warning: {}", e);
                }
            }
        }
        
        files
    }
    
    /// Find and load import.hx files in a directory
    pub fn load_import_hx(&mut self, dir: &Path) -> Vec<HaxeFile> {
        if !self.config.load_import_hx {
            return Vec::new();
        }
        
        let mut files = Vec::new();
        let import_path = dir.join("import.hx");
        
        if import_path.exists() {
            match self.load_file(&import_path) {
                Ok(file) => files.push(file.clone()),
                Err(e) => eprintln!("Warning: Failed to load import.hx: {}", e),
            }
        }
        
        files
    }
}

/// Creates a minimal StdTypes.hx content for bootstrapping
/// This defines the core types that are always available
pub fn create_minimal_std_types() -> &'static str {
    r#"
// Core Haxe standard types - always available without import
package;

// Primitive types
@:coreType abstract Void { }
@:coreType abstract Bool { }
@:coreType abstract Int { }
@:coreType abstract Float { }
@:coreType abstract Dynamic<T> { }

// String is special - it's both a class and has special syntax
@:coreType @:final class String {
    public var length(default, null):Int;
    public function new(string:String) { }
    public function charAt(index:Int):String;
    public function charCodeAt(index:Int):Null<Int>;
    public function indexOf(str:String, ?startIndex:Int):Int;
    public function lastIndexOf(str:String, ?startIndex:Int):Int;
    public function split(delimiter:String):Array<String>;
    public function substr(pos:Int, ?len:Int):String;
    public function substring(startIndex:Int, ?endIndex:Int):String;
    public function toLowerCase():String;
    public function toUpperCase():String;
    public function toString():String;
}

// Core container types
@:coreType abstract Array<T> {
    public var length(default, null):Int;
    public function new():Void;
    public function concat(a:Array<T>):Array<T>;
    public function join(sep:String):String;
    public function pop():Null<T>;
    public function push(x:T):Int;
    public function reverse():Void;
    public function shift():Null<T>;
    public function slice(pos:Int, ?end:Int):Array<T>;
    public function sort(f:T->T->Int):Void;
    public function splice(pos:Int, len:Int):Array<T>;
    public function toString():String;
    public function unshift(x:T):Void;
    
    // Array access
    @:arrayAccess function get(i:Int):T;
    @:arrayAccess function set(i:Int, v:T):T;
}

// Null wrapper
@:coreType abstract Null<T> { }

// Class type
typedef Class<T> = Dynamic;
typedef Enum<T> = Dynamic;

// Function type (simplified)
abstract Function(Dynamic) { }

// Iterator protocol
typedef Iterator<T> = {
    function hasNext():Bool;
    function next():T;
}

typedef Iterable<T> = {
    function iterator():Iterator<T>;
}

// Map type
@:coreType abstract Map<K, V> {
    public function new():Void;
    public function set(key:K, value:V):Void;
    public function get(key:K):Null<V>;
    public function exists(key:K):Bool;
    public function remove(key:K):Bool;
    public function keys():Iterator<K>;
    public function iterator():Iterator<V>;
    public function keyValueIterator():Iterator<{key:K, value:V}>;
    public function clear():Void;
}

// Standard interfaces
interface Comparable<T> {
    public function compareTo(other:T):Int;
}

// Math class (static methods)
@:native("Math")
extern class Math {
    static var PI(default, never):Float;
    static var POSITIVE_INFINITY(default, never):Float;
    static var NEGATIVE_INFINITY(default, never):Float;
    static var NaN(default, never):Float;
    
    static function abs(v:Float):Float;
    static function acos(v:Float):Float;
    static function asin(v:Float):Float;
    static function atan(v:Float):Float;
    static function atan2(y:Float, x:Float):Float;
    static function ceil(v:Float):Int;
    static function cos(v:Float):Float;
    static function exp(v:Float):Float;
    static function floor(v:Float):Int;
    static function log(v:Float):Float;
    static function max(a:Float, b:Float):Float;
    static function min(a:Float, b:Float):Float;
    static function pow(v:Float, exp:Float):Float;
    static function random():Float;
    static function round(v:Float):Int;
    static function sin(v:Float):Float;
    static function sqrt(v:Float):Float;
    static function tan(v:Float):Float;
    
    static inline function isNaN(v:Float):Bool {
        return v != v;
    }
    
    static inline function isFinite(v:Float):Bool {
        return v != POSITIVE_INFINITY && v != NEGATIVE_INFINITY && !isNaN(v);
    }
}

// Std utility class  
class Std {
    public static function int(x:Float):Int;
    public static function parseInt(x:String):Null<Int>;
    public static function parseFloat(x:String):Float;
    public static function string(s:Dynamic):String;
    
    public static inline function is(v:Dynamic, t:Dynamic):Bool {
        return untyped __js__("(v instanceof t)");
    }
    
    public static inline function isOfType(v:Dynamic, t:Dynamic):Bool {
        return is(v, t);
    }
    
    public static inline function downcast<T, S:T>(value:T, c:Class<S>):S {
        return if (is(value, c)) cast value else null;
    }
    
    public static inline function instance<T, S:T>(value:T, c:Class<S>):S {
        return downcast(value, c);
    }
}

// Type utility class
class Type {
    public static function getClass<T>(o:T):Class<Dynamic>;
    public static function getEnum<T>(o:T):Enum<Dynamic>;
    public static function getSuperClass(c:Class<Dynamic>):Class<Dynamic>;
    public static function getClassName(c:Class<Dynamic>):String;
    public static function getEnumName(e:Enum<Dynamic>):String;
    public static function resolveClass(name:String):Class<Dynamic>;
    public static function resolveEnum(name:String):Enum<Dynamic>;
    public static function createInstance<T>(cl:Class<T>, args:Array<Dynamic>):T;
    public static function createEmptyInstance<T>(cl:Class<T>):T;
    public static function createEnum<T>(e:Enum<T>, constr:String, ?params:Array<Dynamic>):T;
    public static function createEnumIndex<T>(e:Enum<T>, index:Int, ?params:Array<Dynamic>):T;
    public static function getInstanceFields(c:Class<Dynamic>):Array<String>;
    public static function getClassFields(c:Class<Dynamic>):Array<String>;
    public static function getEnumConstructs(e:Enum<Dynamic>):Array<String>;
    public static function typeof(v:Dynamic):ValueType;
    public static function enumEq<T>(a:T, b:T):Bool;
    public static function enumConstructor<T>(e:T):String;
    public static function enumParameters<T>(e:T):Array<Dynamic>;
    public static function enumIndex<T>(e:T):Int;
    public static function allEnums<T>(e:Enum<T>):Array<T>;
}

enum ValueType {
    TNull;
    TInt;
    TFloat;
    TBool;
    TObject;
    TFunction;
    TClass(c:Class<Dynamic>);
    TEnum(e:Enum<Dynamic>);
    TUnknown;
}
"#
}