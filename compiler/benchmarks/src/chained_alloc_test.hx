// Minimal test for chained heap allocations in a loop
package benchmarks;

class Point {
    public var x:Float;
    public var y:Float;

    public function new(x:Float, y:Float) {
        this.x = x;
        this.y = y;
    }

    public function add(p:Point):Point {
        return new Point(x + p.x, y + p.y);
    }
}

class ChainedAllocTest {
    public static function main() {
        var sum = 0.0;

        // Run 100 iterations of outer loop
        for (i in 0...100) {
            var p = new Point(1.0, 2.0);

            // Inner loop with reassignment
            for (j in 0...100) {
                var delta = new Point(0.1, 0.1);
                p = p.add(delta);  // Reassignment - old p should be freed
            }

            sum = sum + p.x;
        }

        trace(sum);
    }
}
