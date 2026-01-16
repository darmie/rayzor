// Arithmetic Benchmark
// A simple benchmark for testing basic arithmetic operations
//
// Tests: Integer arithmetic, loop performance, basic control flow

package benchmarks;

class Arithmetic {
    public static function main() {
        var iterations = 10000000;
        var result = 0;

        // Pure Integer arithmetic using while loop
        var i = 0;
        while (i < iterations) {
            result = result + i;
            result = result - (i / 2);
            result = result * 2;
            result = result / 2;
            result = result % 10000;
            i = i + 1;
        }

        trace(result);
    }
}
