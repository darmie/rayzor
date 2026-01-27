// ================================================================
// Haxe Macro Expression Examples
// ================================================================
// Demonstrates macro expression evaluation, the `macro` keyword,
// and compile-time code generation.

package examples;

// ----------------------------------------------------------------
// 1. Basic macro functions — evaluated at compile time
// ----------------------------------------------------------------

class MacroHelpers {
    /// Compile-time constant folding: evaluated during macro expansion,
    /// the call site receives the computed literal.
    macro static function compileTimeAdd(a:Int, b:Int):Int {
        return a + b;
    }

    /// String manipulation at compile time
    macro static function makeGreeting(name:String):String {
        return "Hello, " + name + "!";
    }

    /// Conditional code generation
    macro static function assertPositive(val:Int):Void {
        if (val <= 0) {
            haxe.macro.Context.error("Value must be positive at compile time", haxe.macro.Context.currentPos());
        }
        return val;
    }

    /// Generate a repeated expression N times
    macro static function repeat(n:Int, body:haxe.macro.Expr):haxe.macro.Expr {
        var exprs = [];
        for (i in 0...n) {
            exprs.push(body);
        }
        return macro $b{exprs};
    }

    /// Generate field accessor string at compile time
    macro static function fieldName(e:haxe.macro.Expr):haxe.macro.Expr {
        switch (e.expr) {
            case EField(_, field):
                return macro $v{field};
            default:
                return macro "unknown";
        }
    }
}

// ----------------------------------------------------------------
// 2. Macro-generated lookup tables
// ----------------------------------------------------------------

class LookupGenerator {
    /// Generate a compile-time lookup array for squares of 0..N
    macro static function generateSquares(n:Int):haxe.macro.Expr {
        var values = [];
        for (i in 0...n) {
            values.push(macro $v{i * i});
        }
        return macro $a{values};
    }

    /// Generate a fibonacci sequence at compile time
    macro static function fibonacci(n:Int):haxe.macro.Expr {
        var a = 0;
        var b = 1;
        var values = [];
        for (i in 0...n) {
            values.push(macro $v{a});
            var temp = a + b;
            a = b;
            b = temp;
        }
        return macro $a{values};
    }
}

// ----------------------------------------------------------------
// 3. Type-safe enum generation
// ----------------------------------------------------------------

class EnumMacro {
    /// Generate an enum-like set of integer constants at compile time
    macro static function makeEnum(names:Array<String>):haxe.macro.Expr {
        var fields = [];
        for (i in 0...names.length) {
            var name = names[i];
            fields.push({
                field: name,
                expr: macro $v{i}
            });
        }
        return macro $a{fields};
    }
}

// ----------------------------------------------------------------
// 4. Usage
// ----------------------------------------------------------------

class Main {
    // Compile-time computed constants
    static var TOTAL = MacroHelpers.compileTimeAdd(100, 200);
    static var GREETING = MacroHelpers.makeGreeting("World");

    // Compile-time lookup tables
    static var SQUARES = LookupGenerator.generateSquares(10);
    static var FIB = LookupGenerator.fibonacci(12);

    static function main() {
        // TOTAL is already 300 at compile time — no runtime addition
        trace("Total: " + TOTAL);

        // GREETING is already "Hello, World!" — no runtime concatenation
        trace(GREETING);

        // SQUARES is [0, 1, 4, 9, 16, 25, 36, 49, 64, 81]
        trace("Squares: " + SQUARES);

        // FIB is [0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89]
        trace("Fibonacci: " + FIB);

        // Macro-generated repeated code
        var counter = 0;
        MacroHelpers.repeat(5, counter++);
        trace("Counter after 5 repeats: " + counter); // 5

        // Field name extraction at compile time
        var obj = {x: 10, y: 20};
        var name = MacroHelpers.fieldName(obj.x);
        trace("Field name: " + name); // "x"
    }
}
