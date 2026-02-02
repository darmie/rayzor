// Mandelbrot Benchmark (Class-based)
// Matches official Haxe benchmark at https://benchs.haxe.org/mandelbrot/
// Source: https://github.com/HaxeBenchmarks/benchmark-runner/blob/master/cases/mandelbrot/BMMandelbrotCode.hx
//
// Tests: CPU-intensive computation, floating-point arithmetic, class instantiation

package benchmarks;

class RGB {
    public var r:Int;
    public var g:Int;
    public var b:Int;

    public function new(r:Int, g:Int, b:Int) {
        this.r = r;
        this.g = g;
        this.b = b;
    }
}

class Complex {
    public var i:Float;
    public var j:Float;

    public function new(i:Float, j:Float) {
        this.i = i;
        this.j = j;
    }
}

class Mandelbrot {
    static var SIZE = 25;
    static var MAX_ITER = 1000;
    static var MAX_RAD = 65536;
    static var WIDTH = 875;   // 35 * SIZE
    static var HEIGHT = 500;  // 20 * SIZE

    public static function main() {
        var palette = new Array<RGB>();
        for (idx in 0...1001) {
            palette.push(createPalette(idx / 1000.0));
        }

        var image = new Array<RGB>();
        var outPixel = 0;
        var scale = 0.1 / 25.0;
        var w = 875;
        var h = 500;
        var maxIter = 1000;
        var maxRad = 65536;

        for (y in 0...h) {
            for (x in 0...w) {
                var iteration = 0;

                var offset = new Complex(x * scale - 2.5, y * scale - 1.0);
                var val = new Complex(0.0, 0.0);
                while (complexLength2(val) < maxRad && iteration < maxIter) {
                    val = complexAdd(complexSquare(val), offset);
                    iteration = iteration + 1;
                }

                image.push(palette[iteration]);
            }
        }
    }

    static function complexLength2(val:Complex):Float {
        return val.i * val.i + val.j * val.j;
    }

    static function complexAdd(val0:Complex, val1:Complex):Complex {
        return new Complex(val0.i + val1.i, val0.j + val1.j);
    }

    static function complexSquare(val:Complex):Complex {
        return new Complex(val.i * val.i - val.j * val.j, 2.0 * val.i * val.j);
    }

    static function createPalette(fraction:Float):RGB {
        var r = Std.int(fraction * 255);
        var g = Std.int((1.0 - fraction) * 255);
        var abs_val = fraction - 0.5;
        if (abs_val < 0.0) abs_val = -abs_val;
        var b = Std.int((0.5 - abs_val) * 2.0 * 255);
        return new RGB(r, g, b);
    }
}
