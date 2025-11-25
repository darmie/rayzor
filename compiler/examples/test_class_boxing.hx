package test;

class Point {
    public var x:Int;
    public var y:Int;

    public function new(x:Int, y:Int) {
        this.x = x;
        this.y = y;
    }

    public function toString():String {
        return "Point(" + Std.string(x) + ", " + Std.string(y) + ")";
    }
}

class Main {
    static function main() {
        // Create a Point instance
        var p = new Point(10, 20);

        // Box it to Dynamic
        var d:Dynamic = p;

        // Unbox it back to Point
        var p2:Point = d;

        // Test that it's the same object
        trace(p2.x);  // Should print 10
        trace(p2.y);  // Should print 20
    }
}
