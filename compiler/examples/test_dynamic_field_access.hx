package test;

class Point {
    public var x:Int;
    public var y:Int;

    public function new(x:Int, y:Int) {
        this.x = x;
        this.y = y;
    }
}

class Main {
    static function main() {
        var p = new Point(10, 20);
        var d:Dynamic = p;

        // Try to access fields through Dynamic
        trace(d.x);  // Can we do this?
        trace(d.y);  // Can we do this?
    }
}
