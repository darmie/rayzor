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
    static inline var WIDTH = 875;
    static inline var HEIGHT = 500;
    static inline var MAX_ITER = 1000;

    public static function main() {
        var checksum = 0;

        for (y in 0...HEIGHT) {
            for (x in 0...WIDTH) {
                var c = new Complex(
                    (x - WIDTH / 2) * 4.0 / WIDTH,
                    (y - HEIGHT / 2) * 4.0 / HEIGHT
                );
                checksum = checksum + iterate(c);
            }
        }

        trace(checksum);
    }

    static function iterate(c:Complex):Int {
        var z = new Complex(0.0, 0.0);
        for (i in 0...MAX_ITER) {
            z = z.mul(z).add(c);
            if (z.abs() > 2.0) return i;
        }
        return MAX_ITER;
    }
}
