package test;

class Point {
    public var x:Int;
    public var y:Int;

    public function new(x:Int, y:Int) {
        this.x = x;
        this.y = y;
    }

    public function add(other:Point):Point {
        return new Point(this.x + other.x, this.y + other.y);
    }

    public function getX():Int {
        return this.x;
    }

    public function getY():Int {
        return this.y;
    }
}

class Main {
    static function main() {
        var p1 = new Point(10, 20);
        var p2 = new Point(5, 3);
        var d:Dynamic = p1;

        // Try to call methods through Dynamic
        trace(d.getX());  // Should print 10
        trace(d.getY());  // Should print 20

        // Method with argument
        var result:Point = d.add(p2);
        trace(result.x);  // Should print 15
        trace(result.y);  // Should print 23
    }
}
