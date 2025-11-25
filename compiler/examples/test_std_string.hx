package test;

// Direct extern declarations for testing the runtime type system
@:native("haxe_box_int")
extern function boxInt(value:Int):Dynamic;

@:native("haxe_box_float")
extern function boxFloat(value:Float):Dynamic;

@:native("haxe_box_bool")
extern function boxBool(value:Bool):Dynamic;

@:native("haxe_std_string")
extern function stdString(dynamic:Dynamic):String;

class Main {
    static function main() {
        // Test boxing and Std.string() with runtime type dispatch

        // Box an Int and convert to string
        var dynInt = boxInt(42);
        var strInt = stdString(dynInt);
        trace(strInt);  // Should print "42"

        // Box a Float and convert to string
        var dynFloat = boxFloat(3.14159);
        var strFloat = stdString(dynFloat);
        trace(strFloat);  // Should print "3.14159"

        // Box a Bool and convert to string
        var dynBool = boxBool(true);
        var strBool = stdString(dynBool);
        trace(strBool);  // Should print "true"

        var dynBool2 = boxBool(false);
        var strBool2 = stdString(dynBool2);
        trace(strBool2);  // Should print "false"
    }
}
