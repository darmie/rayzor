// import.hx - Automatic imports for all modules in this directory
// These imports are automatically available to all .hx files in this directory and subdirectories

import haxe.ds.StringMap;
import haxe.ds.IntMap;
import haxe.ds.Option;

// You can also use 'using' for static extensions
using StringTools;
using Lambda;

// Type aliases that will be available everywhere
typedef Vec2 = {x:Float, y:Float};
typedef Vec3 = {x:Float, y:Float, z:Float};

// You can even define common interfaces
interface IDisposable {
    function dispose():Void;
}

// Common utility functions as static extensions
class CommonExtensions {
    public static function isNullOrEmpty(s:String):Bool {
        return s == null || s.length == 0;
    }
    
    public static function clamp(value:Float, min:Float, max:Float):Float {
        return Math.max(min, Math.min(max, value));
    }
}