// Mandelbrot Benchmark (Simple/Inline version)
// A simpler version without class overhead for baseline comparison
//
// Tests: Pure floating-point arithmetic, nested loops

package benchmarks;

class MandelbrotSimple {
    static inline var WIDTH = 875;
    static inline var HEIGHT = 500;
    static inline var MAX_ITER = 1000;

    public static function main() {
        var checksum = 0;

        var y = 0;
        while (y < HEIGHT) {
            var x = 0;
            while (x < WIDTH) {
                var cx = (x - WIDTH / 2) * 4.0 / WIDTH;
                var cy = (y - HEIGHT / 2) * 4.0 / HEIGHT;

                var zx = 0.0;
                var zy = 0.0;
                var iter = 0;

                while (iter < MAX_ITER) {
                    var zx2 = zx * zx;
                    var zy2 = zy * zy;

                    if (zx2 + zy2 > 4.0) {
                        break;
                    }

                    var new_zx = zx2 - zy2 + cx;
                    zy = 2.0 * zx * zy + cy;
                    zx = new_zx;
                    iter = iter + 1;
                }

                checksum = checksum + iter;
                x = x + 1;
            }
            y = y + 1;
        }

        trace(checksum);
    }
}
