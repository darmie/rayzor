// Small Mandelbrot test with Complex class to verify memory management
// Tests: Chained method calls with heap allocations in loops

package benchmarks;

class Complex2 {
    public var re:Float;
    public var im:Float;

    public function new(re:Float, im:Float) {
        this.re = re;
        this.im = im;
    }

    public function add(c:Complex2):Complex2 {
        return new Complex2(re + c.re, im + c.im);
    }

    public function mul(c:Complex2):Complex2 {
        return new Complex2(re * c.re - im * c.im, re * c.im + im * c.re);
    }

    public function abs():Float {
        return Math.sqrt(re * re + im * im);
    }
}

class MandelbrotClassSmall {
    public static function main() {
        var checksum = 0;
        var w = 50;  // Small size for testing
        var h = 50;
        var maxIter = 100;

        for (y in 0...h) {
            for (x in 0...w) {
                var c = new Complex2(
                    (x - 25) * 4.0 / 50,
                    (y - 25) * 4.0 / 50
                );

                // Inline iterate to keep test simple
                var z = new Complex2(0.0, 0.0);
                var iter = 0;
                for (i in 0...maxIter) {
                    z = z.mul(z).add(c);  // This is the critical pattern
                    if (z.abs() > 2.0) {
                        iter = i;
                        break;
                    }
                    iter = maxIter;
                }
                checksum = checksum + iter;
            }
        }

        trace(checksum);
    }
}
