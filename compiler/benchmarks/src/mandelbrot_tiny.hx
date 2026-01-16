package test;

class MandelbrotTiny {
    public static function main() {
        var checksum = 0;

        var y = 0;
        while (y < 10) {
            var x = 0;
            while (x < 10) {
                var cx = (x - 5) * 4.0 / 10;
                var cy = (y - 5) * 4.0 / 10;

                var zx = 0.0;
                var zy = 0.0;
                var iter = 0;

                while (iter < 10) {
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
