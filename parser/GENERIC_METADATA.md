# @:generic Metadata Support

The Haxe parser now fully supports the `@:generic` metadata, which is used to mark types for generic type parameter specialization during compilation.

## Overview

The `@:generic` metadata tells the Haxe compiler to create specialized versions of generic types for each unique combination of type parameters used. This can improve performance by avoiding boxing/unboxing operations and enabling better optimization.

## Supported Declarations

The parser supports `@:generic` metadata on all relevant type declarations:

### Classes
```haxe
@:generic
class GenericClass<T> {
    public var value: T;
    
    public function new(value: T) {
        this.value = value;
    }
}
```

### Interfaces
```haxe
@:generic
interface GenericInterface<T> {
    function process(item: T): T;
}
```

### Abstracts
```haxe
@:generic
abstract GenericAbstract<T>(T) {
    public inline function new(value: T) {
        this = value;
    }
}
```

### Enums
```haxe
@:generic
enum GenericEnum<T> {
    Value(value: T);
    Empty;
}
```

### Typedefs
```haxe
@:generic
typedef GenericTypedef<T> = {
    value: T,
    process: T -> T
}
```

## Features Supported

### Multiple Type Parameters
```haxe
@:generic
class MultiGeneric<T, U, V> {
    public function convert(a: T, b: U): V {
        return cast a;
    }
}
```

### Type Constraints
```haxe
@:generic
class ConstrainedGeneric<T:Iterable<U>, U:Comparable<U>> {
    public function process(items: T): Array<U> {
        return [for (item in items) item];
    }
}
```

### Complex Type Parameters
```haxe
@:generic
class ComplexGeneric<T:{name:String, age:Int}> {
    public function validate(item: T): Bool {
        return item.name != null && item.age > 0;
    }
}
```

### Combined with Other Metadata
```haxe
@:generic
@:native("NativeGeneric")
@:final
class CombinedGeneric<T> {
    public final value: T;
    
    public function new(value: T) {
        this.value = value;
    }
}
```

## AST Representation

The `@:generic` metadata is stored in the `meta` field of type declarations as a `Metadata` struct:

```rust
pub struct Metadata {
    pub name: String,        // "generic"
    pub params: Vec<Expr>,   // Empty for @:generic (no parameters)
    pub span: Span,          // Source location information
}
```

## Implementation Details

### Parser Support
- The metadata is parsed by the `metadata()` function in `haxe_parser.rs`
- Supports both `@:generic` and `@generic` syntax (with and without colon)
- Properly handles whitespace and comments around metadata declarations
- Integrates with the existing metadata parsing infrastructure

### AST Integration
- All relevant type declarations (`ClassDecl`, `InterfaceDecl`, `AbstractDecl`, `EnumDecl`, `TypedefDecl`) include a `meta: Vec<Metadata>` field
- The metadata is preserved with full span information for error reporting and tooling
- Multiple metadata can be applied to the same declaration

### Error Handling
- Syntax errors in metadata are properly reported with source location
- Invalid metadata combinations are detected during parsing
- Provides helpful error messages for common mistakes

## Testing

Comprehensive tests have been added to verify @:generic metadata support:

- `test_generic_metadata_simple`: Basic @:generic on different declaration types
- `test_generic_metadata_with_params`: Generic declarations with type parameters
- `test_generic_metadata_comprehensive`: Complex scenarios with multiple type parameters and constraints
- `test_generic_metadata_edge_cases`: Edge cases like inline declarations and multiple metadata
- `test_generic_metadata_complex_type_params`: Complex type parameter constraints

All tests pass successfully, confirming robust support for @:generic metadata parsing.

## Usage Examples

### Basic Usage
```haxe
@:generic
class Container<T> {
    private var items: Array<T>;
    
    public function new() {
        items = [];
    }
    
    public function add(item: T): Void {
        items.push(item);
    }
    
    public function get(index: Int): T {
        return items[index];
    }
}

// Usage creates specialized versions:
var intContainer = new Container<Int>();    // Container_Int
var stringContainer = new Container<String>(); // Container_String
```

### With Constraints
```haxe
@:generic
class Processor<T:Iterable<String>> {
    public function process(data: T): Array<String> {
        return [for (item in data) item.toUpperCase()];
    }
}
```

### Combined Metadata
```haxe
@:generic
@:native("FastArray")
abstract FastArray<T>(Array<T>) {
    public inline function new() {
        this = [];
    }
    
    @:arrayAccess
    public inline function get(index: Int): T {
        return this[index];
    }
    
    @:arrayAccess
    public inline function set(index: Int, value: T): T {
        return this[index] = value;
    }
}
```

## Notes

- The `@:generic` metadata is purely a compile-time directive and does not affect runtime behavior
- Generic specialization happens during compilation, not at runtime
- The parser preserves all metadata information for use by the compiler backend
- This implementation is fully compatible with Haxe's official @:generic metadata specification