// ================================================================
// Haxe Macro Reification Examples
// ================================================================
// Demonstrates the reification engine: converting between runtime
// values and AST nodes using $v{}, $i{}, $e{}, $a{}, $p{}, $b{}.
//
// Reification syntax reference:
//   $v{value}  — splice a value as a constant expression
//   $i{ident}  — splice a string as an identifier
//   $e{expr}   — splice an expression node directly
//   $a{array}  — splice array elements into an expression array
//   $p{path}   — splice a string as a type/dotted path
//   $b{stmts}  — splice a statement array as a block body

package examples;

import haxe.macro.Expr;
import haxe.macro.Context;

// ----------------------------------------------------------------
// 1. $v{} — Value splicing (constants)
// ----------------------------------------------------------------

class ValueSplice {
    /// Splice a computed integer constant into the AST
    macro static function makeConst(n:Int):Expr {
        var doubled = n * 2;
        return macro $v{doubled};
    }

    /// Splice a computed string
    macro static function makeLabel(prefix:String, id:Int):Expr {
        var label = prefix + "_" + Std.string(id);
        return macro $v{label};
    }

    /// Splice a boolean condition
    macro static function isEven(n:Int):Expr {
        var even = (n % 2 == 0);
        return macro $v{even};
    }
}

// ----------------------------------------------------------------
// 2. $i{} — Identifier splicing
// ----------------------------------------------------------------

class IdentSplice {
    /// Generate access to a variable by dynamic name
    macro static function getVar(name:String):Expr {
        return macro $i{name};
    }

    /// Generate a setter: `varName = value`
    macro static function setVar(name:String, value:Expr):Expr {
        return macro $i{name} = $e{value};
    }

    /// Generate a method call on `this` by name
    macro static function callMethod(name:String):Expr {
        return macro $i{name}();
    }
}

// ----------------------------------------------------------------
// 3. $e{} — Expression splicing
// ----------------------------------------------------------------

class ExprSplice {
    /// Wrap an expression in a null check
    macro static function nullSafe(expr:Expr):Expr {
        return macro {
            var _tmp = $e{expr};
            if (_tmp != null) _tmp else throw "null value";
        };
    }

    /// Wrap an expression with timing
    macro static function timed(label:String, expr:Expr):Expr {
        return macro {
            var _start = Sys.time();
            var _result = $e{expr};
            var _elapsed = Sys.time() - _start;
            trace($v{label} + " took " + _elapsed + "s");
            _result;
        };
    }

    /// Assert that an expression is true, with message
    macro static function assertExpr(expr:Expr, msg:String):Expr {
        return macro {
            if (!($e{expr})) {
                throw "Assertion failed: " + $v{msg};
            }
        };
    }
}

// ----------------------------------------------------------------
// 4. $a{} — Array element splicing
// ----------------------------------------------------------------

class ArraySplice {
    /// Generate an array literal from computed values
    macro static function range(start:Int, end:Int):Expr {
        var values:Array<Expr> = [];
        for (i in start...end) {
            values.push(macro $v{i});
        }
        return macro [$a{values}];
    }

    /// Generate a sum expression: a + b + c + ...
    macro static function sumAll(exprs:Array<Expr>):Expr {
        if (exprs.length == 0) return macro 0;
        var result = exprs[0];
        for (i in 1...exprs.length) {
            result = macro $e{result} + $e{exprs[i]};
        }
        return result;
    }

    /// Interleave expressions with trace calls
    macro static function traceEach(label:String, exprs:Array<Expr>):Expr {
        var stmts:Array<Expr> = [];
        for (i in 0...exprs.length) {
            stmts.push(macro trace($v{label} + "[" + $v{i} + "] = " + $e{exprs[i]}));
        }
        return macro $b{stmts};
    }
}

// ----------------------------------------------------------------
// 5. $p{} — Path splicing (type paths, dotted identifiers)
// ----------------------------------------------------------------

class PathSplice {
    /// Generate a `new` call for a class by name
    macro static function createInstance(className:String):Expr {
        return macro new $p{className}();
    }

    /// Generate a static field access by class and field name
    macro static function staticAccess(className:String, fieldName:String):Expr {
        var path = className + "." + fieldName;
        return macro $p{path};
    }
}

// ----------------------------------------------------------------
// 6. $b{} — Block splicing (statement sequences)
// ----------------------------------------------------------------

class BlockSplice {
    /// Generate a block of variable declarations
    macro static function declareVars(names:Array<String>, value:Expr):Expr {
        var stmts:Array<Expr> = [];
        for (name in names) {
            stmts.push(macro var $i{name} = $e{value});
        }
        return macro $b{stmts};
    }

    /// Generate an unrolled loop
    macro static function unroll(count:Int, body:Expr):Expr {
        var stmts:Array<Expr> = [];
        for (i in 0...count) {
            stmts.push(macro {
                var _i = $v{i};
                $e{body};
            });
        }
        return macro $b{stmts};
    }

    /// Generate a chain of if/else-if conditions
    macro static function cond(conditions:Array<{test:Expr, body:Expr}>):Expr {
        if (conditions.length == 0) return macro null;
        var last = conditions[conditions.length - 1];
        var result = macro if ($e{last.test}) $e{last.body};
        var i = conditions.length - 2;
        while (i >= 0) {
            var c = conditions[i];
            result = macro if ($e{c.test}) $e{c.body} else $e{result};
            i--;
        }
        return result;
    }
}

// ----------------------------------------------------------------
// 7. Combined usage example
// ----------------------------------------------------------------

class Main {
    static function main() {
        // $v{} — computed constants
        trace(ValueSplice.makeConst(21));       // 42
        trace(ValueSplice.makeLabel("btn", 3)); // "btn_3"
        trace(ValueSplice.isEven(4));           // true

        // $i{} — identifier splicing
        var myVar = 100;
        trace(IdentSplice.getVar("myVar")); // 100

        // $e{} — expression wrapping
        ExprSplice.assertExpr(1 + 1 == 2, "basic math");
        trace("Assertion passed");

        // $a{} — array generation
        var r = ArraySplice.range(5, 10);
        trace("Range: " + r); // [5, 6, 7, 8, 9]

        var total = ArraySplice.sumAll([10, 20, 30]);
        trace("Sum: " + total); // 60

        // $b{} — block generation
        BlockSplice.declareVars(["a", "b", "c"], 0);
        trace("Declared a=" + a + " b=" + b + " c=" + c);

        // Unrolled loop
        var sum = 0;
        BlockSplice.unroll(4, sum += _i);
        trace("Unrolled sum: " + sum); // 0+1+2+3 = 6
    }
}
