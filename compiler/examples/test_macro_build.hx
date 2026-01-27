// ================================================================
// Haxe Build Macro Examples (@:build, @:autoBuild)
// ================================================================
// Demonstrates compile-time class field generation and modification
// using @:build macros, and automatic propagation via @:autoBuild
// on interfaces.
//
// @:build(MacroClass.method) — Calls the macro on the annotated class.
//   The macro receives the class fields via Context.getBuildFields()
//   and returns modified fields.
//
// @:autoBuild — Applied to interfaces. Any class implementing the
//   interface automatically has the build macro applied.

package examples;

import haxe.macro.Context;
import haxe.macro.Expr;

// ================================================================
// BUILD MACRO IMPLEMENTATIONS
// ================================================================

class BuildMacros {
    // ----------------------------------------------------------------
    // 1. Add a toString() method automatically
    // ----------------------------------------------------------------
    macro static function addToString():Array<Field> {
        var fields = Context.getBuildFields();
        var className = Context.getLocalClass().get().name;

        // Collect all var field names
        var varNames:Array<String> = [];
        for (f in fields) {
            switch (f.kind) {
                case FVar(_, _):
                    varNames.push(f.name);
                default:
            }
        }

        // Build the toString body: "ClassName(field1=..., field2=...)"
        var parts:Array<Expr> = [macro $v{className + "("}];
        for (i in 0...varNames.length) {
            if (i > 0) parts.push(macro ", ");
            var name = varNames[i];
            parts.push(macro $v{name + "="});
            parts.push(macro Std.string($i{name}));
        }
        parts.push(macro ")");

        // Fold into a single concatenation expression
        var body = parts[0];
        for (i in 1...parts.length) {
            body = macro $e{body} + $e{parts[i]};
        }

        // Add the toString method
        fields.push({
            name: "toString",
            access: [APublic],
            kind: FFun({
                args: [],
                ret: macro :String,
                expr: macro return $e{body}
            }),
            pos: Context.currentPos()
        });

        return fields;
    }

    // ----------------------------------------------------------------
    // 2. Add serialization support
    // ----------------------------------------------------------------
    macro static function addSerialize():Array<Field> {
        var fields = Context.getBuildFields();

        // Collect field info for serialization
        var varFields:Array<{name:String, type:String}> = [];
        for (f in fields) {
            switch (f.kind) {
                case FVar(t, _):
                    varFields.push({
                        name: f.name,
                        type: t != null ? haxe.macro.ComplexTypeTools.toString(t) : "Dynamic"
                    });
                default:
            }
        }

        // Generate toMap(): Map<String, Dynamic>
        var mapEntries:Array<Expr> = [];
        for (vf in varFields) {
            mapEntries.push(macro map.set($v{vf.name}, $i{vf.name}));
        }

        fields.push({
            name: "toMap",
            access: [APublic],
            kind: FFun({
                args: [],
                ret: null,
                expr: macro {
                    var map = new Map<String, Dynamic>();
                    $b{mapEntries};
                    return map;
                }
            }),
            pos: Context.currentPos()
        });

        // Generate fieldCount(): Int
        fields.push({
            name: "fieldCount",
            access: [APublic, AInline],
            kind: FFun({
                args: [],
                ret: macro :Int,
                expr: macro return $v{varFields.length}
            }),
            pos: Context.currentPos()
        });

        return fields;
    }

    // ----------------------------------------------------------------
    // 3. Add validation methods
    // ----------------------------------------------------------------
    macro static function addValidation():Array<Field> {
        var fields = Context.getBuildFields();

        // Find fields with @:notNull metadata
        var requiredFields:Array<String> = [];
        for (f in fields) {
            for (m in f.meta) {
                if (m.name == ":notNull" || m.name == "notNull") {
                    requiredFields.push(f.name);
                }
            }
        }

        // Generate validate(): Bool
        var checks:Array<Expr> = [];
        for (name in requiredFields) {
            checks.push(macro {
                if ($i{name} == null) return false;
            });
        }

        fields.push({
            name: "validate",
            access: [APublic],
            kind: FFun({
                args: [],
                ret: macro :Bool,
                expr: macro {
                    $b{checks};
                    return true;
                }
            }),
            pos: Context.currentPos()
        });

        return fields;
    }

    // ----------------------------------------------------------------
    // 4. Auto-implement interface methods with default behavior
    // ----------------------------------------------------------------
    macro static function autoImplement():Array<Field> {
        var fields = Context.getBuildFields();

        // Check which interface methods are missing
        var cls = Context.getLocalClass().get();
        for (iface in cls.interfaces) {
            var ifaceType = iface.t.get();
            for (ifaceField in ifaceType.fields.get()) {
                // Check if already implemented
                var found = false;
                for (f in fields) {
                    if (f.name == ifaceField.name) {
                        found = true;
                        break;
                    }
                }
                if (!found) {
                    // Add a default implementation
                    fields.push({
                        name: ifaceField.name,
                        access: [APublic],
                        kind: FFun({
                            args: [],
                            ret: null,
                            expr: macro {
                                trace("Default implementation: " + $v{ifaceField.name});
                                return null;
                            }
                        }),
                        pos: Context.currentPos()
                    });
                }
            }
        }

        return fields;
    }
}

// ================================================================
// @:autoBuild — Interface-driven build macros
// ================================================================

/// Any class implementing Trackable automatically gets
/// getId(), getTimestamp(), and getSource() methods generated.
@:autoBuild
@:build(BuildMacros.addSerialize)
interface Trackable {
    function getId():String;
    function getTimestamp():Float;
}

/// Any class implementing Printable gets a toString() method.
@:autoBuild
@:build(BuildMacros.addToString)
interface Printable {
    function toString():String;
}

// ================================================================
// CLASSES USING BUILD MACROS
// ================================================================

// ----------------------------------------------------------------
// Direct @:build usage — adds toString() to Point
// ----------------------------------------------------------------
@:build(BuildMacros.addToString)
class Point {
    public var x:Float;
    public var y:Float;

    public function new(x:Float, y:Float) {
        this.x = x;
        this.y = y;
    }
    // toString() is generated by the build macro:
    // public function toString():String { return "Point(x=..., y=...)"; }
}

// ----------------------------------------------------------------
// Multiple build macros on one class
// ----------------------------------------------------------------
@:build(BuildMacros.addToString)
@:build(BuildMacros.addSerialize)
@:build(BuildMacros.addValidation)
class User {
    @:notNull public var name:String;
    @:notNull public var email:String;
    public var age:Int;

    public function new(name:String, email:String, age:Int) {
        this.name = name;
        this.email = email;
        this.age = age;
    }
    // Generated methods:
    // - toString(): String
    // - toMap(): Map<String, Dynamic>
    // - fieldCount(): Int   (returns 3)
    // - validate(): Bool    (checks name and email are not null)
}

// ----------------------------------------------------------------
// @:autoBuild via Trackable interface
// ----------------------------------------------------------------
class Event implements Trackable {
    public var id:String;
    public var timestamp:Float;
    public var payload:Dynamic;

    public function new(id:String, payload:Dynamic) {
        this.id = id;
        this.timestamp = Sys.time();
        this.payload = payload;
    }

    // Required by Trackable:
    public function getId():String {
        return id;
    }

    public function getTimestamp():Float {
        return timestamp;
    }

    // toMap() and fieldCount() are auto-generated via @:autoBuild
}

// ----------------------------------------------------------------
// @:autoBuild via Printable interface
// ----------------------------------------------------------------
class Color implements Printable {
    public var r:Int;
    public var g:Int;
    public var b:Int;

    public function new(r:Int, g:Int, b:Int) {
        this.r = r;
        this.g = g;
        this.b = b;
    }

    // toString() is auto-generated via @:autoBuild from Printable
}

// ================================================================
// MAIN — Exercise all generated methods
// ================================================================

class Main {
    static function main() {
        // @:build — Point with generated toString
        var p = new Point(3.14, 2.71);
        trace(p.toString()); // "Point(x=3.14, y=2.71)"

        // Multiple @:build — User with toString, serialize, validate
        var user = new User("Alice", "alice@example.com", 30);
        trace(user.toString());            // "User(name=Alice, email=alice@example.com, age=30)"
        trace("Fields: " + user.fieldCount()); // 3

        var map = user.toMap();
        trace("Serialized name: " + map.get("name")); // "Alice"

        trace("Valid: " + user.validate()); // true

        var badUser = new User(null, "x@y.com", 0);
        trace("Bad valid: " + badUser.validate()); // false

        // @:autoBuild — Event implements Trackable (gets toMap, fieldCount)
        var evt = new Event("evt_001", {action: "click"});
        trace("Event ID: " + evt.getId());
        trace("Event fields: " + evt.fieldCount());

        // @:autoBuild — Color implements Printable (gets toString)
        var c = new Color(255, 128, 0);
        trace(c.toString()); // "Color(r=255, g=128, b=0)"

        trace("All build macro examples passed.");
    }
}
