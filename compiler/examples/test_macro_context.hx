// ================================================================
// Haxe Macro Context API Examples
// ================================================================
// Demonstrates haxe.macro.Context methods used within macro
// functions to interact with the compiler during expansion.
//
// Context API reference:
//   Context.error(msg, pos)         — Emit compile error, abort macro
//   Context.warning(msg, pos)       — Emit compile warning
//   Context.info(msg, pos)          — Emit compile info
//   Context.currentPos()            — Get position of macro call site
//   Context.getLocalClass()         — Get the class containing the macro call
//   Context.getLocalMethod()        — Get the method containing the macro call
//   Context.getLocalModule()        — Get the module path of the call site
//   Context.getBuildFields()        — Get class fields (in @:build macros)
//   Context.getType(name)           — Resolve a type by name
//   Context.typeof(expr)            — Infer the type of an expression
//   Context.defineType(td)          — Define a new type at compile time
//   Context.defined(flag)           — Check if a -D flag is defined
//   Context.definedValue(flag)      — Get the value of a -D flag
//   Context.parse(code, pos)        — Parse a string as a Haxe expression

package examples;

import haxe.macro.Context;
import haxe.macro.Expr;
import haxe.macro.Type;

// ================================================================
// 1. DIAGNOSTIC METHODS — error(), warning(), info()
// ================================================================

class DiagnosticMacros {
    /// Emit a compile-time error if the value is negative
    macro static function ensurePositive(value:Int):Expr {
        if (value < 0) {
            Context.error(
                "Value must be non-negative, got " + Std.string(value),
                Context.currentPos()
            );
        }
        return macro $v{value};
    }

    /// Emit a compile-time warning for deprecated usage
    macro static function deprecatedApi(replacement:String, expr:Expr):Expr {
        Context.warning(
            "This API is deprecated. Use '" + replacement + "' instead.",
            Context.currentPos()
        );
        return expr;
    }

    /// Emit a compile-time info message about what's being generated
    macro static function traced(label:String, expr:Expr):Expr {
        Context.info(
            "Generating traced expression: " + label,
            Context.currentPos()
        );
        return macro {
            trace($v{label} + ": entering");
            var _result = $e{expr};
            trace($v{label} + ": exiting with " + Std.string(_result));
            _result;
        };
    }
}

// ================================================================
// 2. POSITION METHODS — currentPos()
// ================================================================

class PositionMacros {
    /// Generate a debug string with source position info
    macro static function here():Expr {
        var pos = Context.currentPos();
        var posInfo = Context.getPosInfos(pos);
        var file = posInfo.file;
        var line = posInfo.min; // line info from position
        return macro $v{file + ":" + Std.string(line)};
    }

    /// Assert with source position embedded in error message
    macro static function assertHere(cond:Expr, msg:String):Expr {
        var pos = Context.currentPos();
        var posInfo = Context.getPosInfos(pos);
        return macro {
            if (!($e{cond})) {
                throw $v{"Assertion failed at " + posInfo.file + ": " + msg};
            }
        };
    }
}

// ================================================================
// 3. LOCAL CONTEXT — getLocalClass(), getLocalMethod()
// ================================================================

class LocalContextMacros {
    /// Return the name of the class where this macro is called
    macro static function className():Expr {
        var cls = Context.getLocalClass();
        if (cls != null) {
            return macro $v{cls.get().name};
        }
        return macro "unknown";
    }

    /// Return the name of the method where this macro is called
    macro static function methodName():Expr {
        var method = Context.getLocalMethod();
        if (method != null) {
            return macro $v{method};
        }
        return macro "unknown";
    }

    /// Return "ClassName.methodName" for logging
    macro static function qualifiedName():Expr {
        var cls = Context.getLocalClass();
        var method = Context.getLocalMethod();
        var clsName = cls != null ? cls.get().name : "?";
        var methodName = method != null ? method : "?";
        return macro $v{clsName + "." + methodName};
    }

    /// Generate a log prefix with class and method context
    macro static function logPrefix():Expr {
        var cls = Context.getLocalClass();
        var method = Context.getLocalMethod();
        var module = Context.getLocalModule();

        var prefix = "[";
        if (module != null) prefix += module + "/";
        if (cls != null) prefix += cls.get().name;
        if (method != null) prefix += "." + method;
        prefix += "]";

        return macro $v{prefix};
    }
}

// ================================================================
// 4. TYPE INTROSPECTION — getType(), typeof()
// ================================================================

class TypeMacros {
    /// Check at compile time whether a type exists
    macro static function typeExists(typeName:String):Expr {
        try {
            Context.getType(typeName);
            return macro true;
        } catch (e:Dynamic) {
            return macro false;
        }
    }

    /// Get the number of fields on a type at compile time
    macro static function fieldCount(typeName:String):Expr {
        var type = Context.getType(typeName);
        switch (type) {
            case TInst(cls, _):
                var fields = cls.get().fields.get();
                return macro $v{fields.length};
            default:
                return macro 0;
        }
    }

    /// Check if an expression has a specific type
    macro static function isType(expr:Expr, typeName:String):Expr {
        var t = Context.typeof(expr);
        var expected = Context.getType(typeName);
        return macro $v{Type.enumEq(t, expected)};
    }

    /// Generate a runtime type name string from a compile-time type
    macro static function nameOf(typeName:String):Expr {
        var type = Context.getType(typeName);
        switch (type) {
            case TInst(cls, _):
                return macro $v{cls.get().name};
            case TEnum(e, _):
                return macro $v{e.get().name};
            case TAbstract(a, _):
                return macro $v{a.get().name};
            default:
                return macro "unknown";
        }
    }
}

// ================================================================
// 5. BUILD FIELDS — getBuildFields() (for @:build macros)
// ================================================================

class BuildFieldMacros {
    /// @:build macro: add a describe() method listing all fields
    macro static function addDescribe():Array<Field> {
        var fields = Context.getBuildFields();
        var className = Context.getLocalClass().get().name;

        // Collect field info
        var descriptions:Array<String> = [];
        for (f in fields) {
            var desc = f.name + ":";
            switch (f.kind) {
                case FVar(t, _):
                    desc += t != null
                        ? haxe.macro.ComplexTypeTools.toString(t)
                        : "Dynamic";
                case FFun(_):
                    desc += "Function";
                case FProp(_, _, t, _):
                    desc += t != null
                        ? haxe.macro.ComplexTypeTools.toString(t)
                        : "Dynamic";
            }
            descriptions.push(desc);
        }

        var descStr = className + " { " + descriptions.join(", ") + " }";

        fields.push({
            name: "describe",
            access: [APublic, AStatic],
            kind: FFun({
                args: [],
                ret: macro :String,
                expr: macro return $v{descStr}
            }),
            pos: Context.currentPos()
        });

        return fields;
    }

    /// @:build macro: add clone() method that copies all var fields
    macro static function addClone():Array<Field> {
        var fields = Context.getBuildFields();
        var className = Context.getLocalClass().get().name;

        // Collect var fields for cloning
        var assignments:Array<Expr> = [];
        for (f in fields) {
            switch (f.kind) {
                case FVar(_, _):
                    var name = f.name;
                    assignments.push(macro obj.$name = this.$name);
                default:
            }
        }

        // Generate: function clone():ClassName {
        //     var obj = new ClassName();
        //     obj.field1 = this.field1;
        //     ...
        //     return obj;
        // }
        fields.push({
            name: "clone",
            access: [APublic],
            kind: FFun({
                args: [],
                ret: null,
                expr: macro {
                    var obj = Type.createEmptyInstance(Type.resolveClass($v{className}));
                    $b{assignments};
                    return obj;
                }
            }),
            pos: Context.currentPos()
        });

        return fields;
    }
}

// ================================================================
// 6. TYPE DEFINITION — defineType()
// ================================================================

class TypeGenMacros {
    /// Generate a companion "Builder" class at compile time
    macro static function generateBuilder(typeName:String):Expr {
        var type = Context.getType(typeName);
        switch (type) {
            case TInst(cls, _):
                var clsType = cls.get();
                var builderName = clsType.name + "Builder";

                // Create builder fields (one per class field)
                var builderFields:Array<Field> = [];
                for (f in clsType.fields.get()) {
                    // Add a setter method for each field
                    var fieldName = f.name;
                    builderFields.push({
                        name: "set" + fieldName.charAt(0).toUpperCase() + fieldName.substr(1),
                        access: [APublic],
                        kind: FFun({
                            args: [{name: "value", type: null}],
                            ret: null,
                            expr: macro {
                                this.$fieldName = value;
                                return this;
                            }
                        }),
                        pos: Context.currentPos()
                    });
                }

                // Define the builder type
                Context.defineType({
                    pack: clsType.pack,
                    name: builderName,
                    pos: Context.currentPos(),
                    kind: TDClass(null, [], false),
                    fields: builderFields
                });

                return macro $v{builderName + " generated"};
            default:
                Context.error("generateBuilder requires a class type", Context.currentPos());
                return macro null;
        }
    }
}

// ================================================================
// 7. CONDITIONAL COMPILATION — defined(), definedValue()
// ================================================================

class ConditionalMacros {
    /// Check a -D flag at macro expansion time
    macro static function ifDefined(flag:String, thenExpr:Expr, elseExpr:Expr):Expr {
        if (Context.defined(flag)) {
            return thenExpr;
        }
        return elseExpr;
    }

    /// Get a -D flag value with a default
    macro static function getDefine(flag:String, defaultValue:String):Expr {
        var val = Context.definedValue(flag);
        if (val != null) {
            return macro $v{val};
        }
        return macro $v{defaultValue};
    }

    /// Generate debug-mode-only code
    macro static function debugOnly(expr:Expr):Expr {
        if (Context.defined("debug")) {
            return expr;
        }
        return macro {};
    }
}

// ================================================================
// 8. PARSE — Context.parse()
// ================================================================

class ParseMacros {
    /// Parse a runtime string as code at compile time
    macro static function eval(code:String):Expr {
        return Context.parse(code, Context.currentPos());
    }

    /// Generate a function body from a string template
    macro static function fromTemplate(template:String, name:String):Expr {
        var code = StringTools.replace(template, "{{name}}", name);
        return Context.parse(code, Context.currentPos());
    }
}

// ================================================================
// CLASSES USING CONTEXT-POWERED MACROS
// ================================================================

@:build(BuildFieldMacros.addDescribe)
@:build(BuildFieldMacros.addClone)
class Config {
    public var host:String;
    public var port:Int;
    public var debug:Bool;

    public function new(host:String, port:Int, debug:Bool) {
        this.host = host;
        this.port = port;
        this.debug = debug;
    }
}

// ================================================================
// MAIN — Exercise all Context API features
// ================================================================

class Main {
    static function main() {
        // 1. Diagnostic macros
        var val = DiagnosticMacros.ensurePositive(42);
        trace("Positive value: " + val);

        // DiagnosticMacros.ensurePositive(-1); // Would emit compile error

        var result = DiagnosticMacros.traced("compute", 2 + 3);
        trace("Traced result: " + result);

        // 2. Position macros
        var pos = PositionMacros.here();
        trace("Called from: " + pos);

        PositionMacros.assertHere(1 + 1 == 2, "basic math");

        // 3. Local context
        trace("Class: " + LocalContextMacros.className());
        trace("Method: " + LocalContextMacros.methodName());
        trace("Qualified: " + LocalContextMacros.qualifiedName());
        trace("Log prefix: " + LocalContextMacros.logPrefix());

        // 4. Type introspection
        trace("String exists: " + TypeMacros.typeExists("String"));
        trace("Config fields: " + TypeMacros.fieldCount("Config"));
        trace("Type name: " + TypeMacros.nameOf("Config"));

        // 5. Build field macros
        trace("Config description: " + Config.describe());

        var cfg = new Config("localhost", 8080, true);
        var cfgClone = cfg.clone();
        trace("Cloned host: " + cfgClone.host);

        // 6. Conditional compilation
        var mode = ConditionalMacros.getDefine("mode", "production");
        trace("Mode: " + mode);

        // 7. Parse macros
        var parsed = ParseMacros.eval("1 + 2 + 3");
        trace("Parsed result: " + parsed);

        var greeting = ParseMacros.fromTemplate(
            "trace('Hello, {{name}}!')",
            "World"
        );

        trace("All Context API examples passed.");
    }
}
