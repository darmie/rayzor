// Mandelbrot Benchmark (Class-based)
// Based on https://benchs.haxe.org/mandelbrot/index.html
//
// Tests: CPU-intensive computation, floating-point arithmetic, class instantiation

package benchmarks;

class Complex {
    public var re:Float;
    public var im:Float;

    public function new(re:Float, im:Float) {
        this.re = re;
        this.im = im;
    }

    public function add(c:Complex):Complex {
        return new Complex(re + c.re, im + c.im);
    }

    public function mul(c:Complex):Complex {
        return new Complex(re * c.re - im * c.im, re * c.im + im * c.re);
    }

    public function abs():Float {
        return Math.sqrt(re * re + im * im);
    }
}

class Mandelbrot {
    // WORKAROUND: Using literal values directly in loops due to static var + loop bound bug
    // Original values: WIDTH = 875, HEIGHT = 500, MAX_ITER = 1000

    public static function main() {
        var checksum = 0;

        // Using literal 500 for HEIGHT, 875 for WIDTH
        for (y in 0...500) {
            for (x in 0...875) {
                var c = new Complex(
                    (x - 437) * 4.0 / 875,    // 437 = 875/2
                    (y - 250) * 4.0 / 500     // 250 = 500/2
                );
                checksum = checksum + iterate(c);
            }
        }

        trace(checksum);
    }

    static function iterate(c:Complex):Int {
        var z = new Complex(0.0, 0.0);
        // Using literal 1000 for MAX_ITER
        for (i in 0...1000) {
            z = z.mul(z).add(c);
            if (z.abs() > 2.0) return i;
        }
        return 1000;
    }
}
