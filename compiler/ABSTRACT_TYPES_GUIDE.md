# Abstract Types Guide

## Overview

Abstract types are one of Haxe's most powerful features, providing type-safe wrappers around underlying types with zero runtime cost. The Rayzor compiler has full support for abstract types including:

- Type-safe wrappers
- Implicit conversions (from/to)
- Operator overloading
- Static methods and fields
- Core types (@:coreType)

## What Are Abstract Types?

Abstract types allow you to create new types that are distinct at compile-time but use an existing type's representation at runtime. Think of them as "branded" or "newtype" wrappers.

### Benefits

1. **Type Safety**: Prevent mixing incompatible values (e.g., `UserId` vs `OrderId`)
2. **Zero Cost**: No runtime overhead - compiled to underlying type
3. **Domain Modeling**: Express business logic in types
4. **API Design**: Create clear, self-documenting interfaces

## Basic Usage

### Simple Wrapper

```haxe
package types;

abstract Counter(Int) {
    public inline function new(value:Int) {
        this = value;
    }

    public inline function increment():Counter {
        return new Counter(this + 1);
    }

    public inline function getValue():Int {
        return this;
    }
}

class Main {
    public static function main():Void {
        var counter = new Counter(0);
        counter = counter.increment();
        var value = counter.getValue();  // 1
    }
}
```

**Key Points:**
- `(Int)` specifies the underlying type
- `this` refers to the underlying value
- Methods can be `inline` for zero runtime cost
- Constructor `new()` initializes the underlying value

## Implicit Conversions

### From Conversions

Allow automatic conversion **from** other types:

```haxe
abstract Kilometers(Float) from Float {
    public inline function new(value:Float) {
        this = value;
    }
}

class Main {
    public static function main():Void {
        var distance:Kilometers = 5.5;  // Implicit conversion from Float
    }
}
```

### To Conversions

Allow automatic conversion **to** other types:

```haxe
abstract Kilometers(Float) to Float {
    public inline function new(value:Float) {
        this = value;
    }
}

class Main {
    public static function main():Void {
        var distance = new Kilometers(5.5);
        var asFloat:Float = distance;  // Implicit conversion to Float
    }
}
```

### Both Directions

```haxe
abstract Kilometers(Float) from Float to Float {
    public inline function new(value:Float) {
        this = value;
    }

    public inline function toMeters():Float {
        return this * 1000;
    }
}

class Main {
    public static function main():Void {
        var distance:Kilometers = 5.5;      // from Float
        var asFloat:Float = distance;        // to Float
        var meters = distance.toMeters();    // method call
    }
}
```

## Operator Overloading

Define how operators work on your abstract type:

```haxe
abstract Vector2D(Array<Float>) {
    public inline function new(x:Float, y:Float) {
        this = [x, y];
    }

    @:op(A + B)
    public inline function add(rhs:Vector2D):Vector2D {
        return new Vector2D(
            this[0] + rhs.getX(),
            this[1] + rhs.getY()
        );
    }

    @:op(A * B)
    public inline function multiply(scalar:Float):Vector2D {
        return new Vector2D(this[0] * scalar, this[1] * scalar);
    }

    public inline function getX():Float return this[0];
    public inline function getY():Float return this[1];
}

class Main {
    public static function main():Void {
        var v1 = new Vector2D(1.0, 2.0);
        var v2 = new Vector2D(3.0, 4.0);
        var sum = v1 + v2;           // Uses @:op(A + B)
        var scaled = v1 * 2.0;       // Uses @:op(A * B)
    }
}
```

### Supported Operators

- `@:op(A + B)` - Addition
- `@:op(A - B)` - Subtraction
- `@:op(A * B)` - Multiplication
- `@:op(A / B)` - Division
- `@:op(A % B)` - Modulo
- `@:op(A == B)` - Equality
- `@:op(A != B)` - Inequality
- `@:op(A < B)` - Less than
- `@:op(A > B)` - Greater than
- `@:op(A <= B)` - Less than or equal
- `@:op(A >= B)` - Greater than or equal
- `@:op(A++)` - Post-increment
- `@:op(++A)` - Pre-increment
- `@:op(A--)` - Post-decrement
- `@:op(--A)` - Pre-decrement
- `@:op(-A)` - Unary negation

## Real-World Examples

### Example 1: Time Units

```haxe
package time;

abstract Milliseconds(Int) from Int to Int {
    public inline function new(value:Int) {
        this = value;
    }

    public inline function toSeconds():Seconds {
        return new Seconds(this / 1000);
    }

    @:op(A + B)
    public inline function add(rhs:Milliseconds):Milliseconds {
        return new Milliseconds(this + rhs.toInt());
    }

    public inline function toInt():Int {
        return this;
    }
}

abstract Seconds(Int) from Int to Int {
    public inline function new(value:Int) {
        this = value;
    }

    public inline function toMilliseconds():Milliseconds {
        return new Milliseconds(this * 1000);
    }

    public inline function toInt():Int {
        return this;
    }
}

class Timer {
    private var elapsed:Milliseconds;

    public function new() {
        this.elapsed = 0;  // Implicit from Int
    }

    public function addTime(ms:Milliseconds):Void {
        elapsed = elapsed + ms;  // Uses @:op
    }

    public function getElapsedSeconds():Seconds {
        return elapsed.toSeconds();
    }
}
```

**Benefits:**
- Can't accidentally mix milliseconds and seconds
- Clear API: `addTime(ms:Milliseconds)` vs `addTime(value:Int)`
- Type-safe conversions between units

### Example 2: Branded IDs

```haxe
package domain;

abstract UserId(Int) from Int to Int {
    public inline function new(value:Int) {
        this = value;
    }

    public inline function toInt():Int {
        return this;
    }
}

abstract OrderId(Int) from Int to Int {
    public inline function new(value:Int) {
        this = value;
    }

    public inline function toInt():Int {
        return this;
    }
}

class User {
    public var id:UserId;
    public var name:String;

    public function new(id:UserId, name:String) {
        this.id = id;
        this.name = name;
    }
}

class Order {
    public var id:OrderId;
    public var userId:UserId;

    public function new(id:OrderId, userId:UserId) {
        this.id = id;
        this.userId = userId;
    }
}

class Main {
    public static function main():Void {
        var user = new User(1, "Alice");    // 1 converts to UserId
        var order = new Order(100, user.id);  // Correct types

        // This would be a compile error:
        // var wrongOrder = new Order(user.id, 100);  // Type error!
    }
}
```

**Benefits:**
- Impossible to mix up user IDs and order IDs
- Self-documenting code
- Catch errors at compile time

### Example 3: Validated Strings

```haxe
package validation;

abstract Email(String) {
    public inline function new(value:String) {
        this = value;
    }

    public static function validate(value:String):Bool {
        return value.indexOf("@") > 0 && value.indexOf(".") > 0;
    }

    public static function create(value:String):Email {
        if (validate(value)) {
            return new Email(value);
        }
        throw "Invalid email: " + value;
    }

    public inline function toString():String {
        return this;
    }
}

class User {
    private var email:Email;

    public function new(emailStr:String) {
        this.email = Email.create(emailStr);  // Validates
    }

    public function getEmail():String {
        return email.toString();
    }
}
```

**Benefits:**
- Validation happens at construction
- Can't create invalid emails
- Type system enforces validation

## Core Types

For primitive types defined by the compiler:

```haxe
@:coreType abstract Void {}
@:coreType abstract Float from Int {}
@:coreType abstract Int to Float {}
```

The `@:coreType` metadata indicates the type is a compiler primitive with no underlying type in source code.

## Static Methods

Abstract types can have static methods:

```haxe
abstract Point(Array<Float>) {
    public inline function new(x:Float, y:Float) {
        this = [x, y];
    }

    public static function zero():Point {
        return new Point(0, 0);
    }

    public static function distance(a:Point, b:Point):Float {
        var dx = a.getX() - b.getX();
        var dy = a.getY() - b.getY();
        return Math.sqrt(dx * dx + dy * dy);
    }

    public inline function getX():Float return this[0];
    public inline function getY():Float return this[1];
}

class Main {
    public static function main():Void {
        var p1 = Point.zero();
        var p2 = new Point(3, 4);
        var dist = Point.distance(p1, p2);  // 5.0
    }
}
```

## Best Practices

### 1. Use `inline` for Performance

```haxe
// Good - zero runtime cost
public inline function getValue():Int {
    return this;
}

// Less optimal - function call overhead
public function getValue():Int {
    return this;
}
```

### 2. Provide Conversion Methods

```haxe
abstract Meters(Float) from Float to Float {
    public inline function new(value:Float) {
        this = value;
    }

    // Explicit conversions when needed
    public inline function toKilometers():Kilometers {
        return new Kilometers(this / 1000);
    }

    public inline function toFloat():Float {
        return this;
    }
}
```

### 3. Smart Constructors for Validation

```haxe
abstract PositiveInt(Int) {
    private inline function new(value:Int) {
        this = value;
    }

    public static function create(value:Int):PositiveInt {
        if (value > 0) {
            return new PositiveInt(value);
        }
        throw "Value must be positive";
    }
}
```

### 4. Operator Overloading for Natural Syntax

```haxe
abstract Money(Int) {
    public inline function new(cents:Int) {
        this = cents;
    }

    @:op(A + B)
    public inline function add(rhs:Money):Money {
        return new Money(this + rhs.toCents());
    }

    @:op(A * B)
    public inline function multiply(factor:Float):Money {
        return new Money(Std.int(this * factor));
    }

    public inline function toCents():Int return this;
}
```

## Compiler Implementation Status

The Rayzor compiler supports:

✅ **Fully Implemented:**
- Abstract type declarations
- Underlying type specification
- from/to conversions (parsed and stored in TAST)
- Methods and fields
- Constructors
- Static methods
- @:coreType abstracts
- Inline methods
- Operator metadata (@:op)

⚠️ **Partial:**
- Implicit cast type checking (structure ready, checking TBD)
- Operator method resolution (parsed but runtime not implemented)

🔄 **In Progress:**
- Full runtime support for operators
- Type unification with implicit casts
- Generic abstract types

## Testing Abstract Types

### Compilation Test

```rust
use compiler::compilation::{CompilationUnit, CompilationConfig};

fn test_abstract() {
    let mut unit = CompilationUnit::new(CompilationConfig::default());

    let source = r#"
        abstract Counter(Int) {
            public inline function new(value:Int) {
                this = value;
            }
        }

        class Main {
            public static function main():Void {
                var c = new Counter(0);
            }
        }
    "#;

    unit.add_file(source, "test.hx").unwrap();
    let typed_files = unit.lower_to_tast().unwrap();

    // Check abstract was lowered
    assert!(!typed_files[0].abstracts.is_empty());
}
```

## See Also

- [Haxe Manual - Abstract Types](https://haxe.org/manual/types-abstract.html)
- [COMPILATION_UNIT_GUIDE.md](COMPILATION_UNIT_GUIDE.md) - Multi-file compilation
- [test_abstract_types.rs](examples/test_abstract_types.rs) - Basic examples
- [test_abstract_real_world.rs](examples/test_abstract_real_world.rs) - Real-world patterns
