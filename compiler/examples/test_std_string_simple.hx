package test;

class Main {
    static function main() {
        // Test Std.string() with compile-time known types

        var s1 = Std.string(42);
        trace(s1);  // Should print "42"

        var s2 = Std.string(3.14159);
        trace(s2);  // Should print "3.14159"

        var s3 = Std.string(true);
        trace(s3);  // Should print "true"

        var s4 = Std.string(false);
        trace(s4);  // Should print "false"

        // Test with variables
        var x = 100;
        var sx = Std.string(x);
        trace(sx);  // Should print "100"

        var y = 2.718;
        var sy = Std.string(y);
        trace(sy);  // Should print "2.718"

        var z = true;
        var sz = Std.string(z);
        trace(sz);  // Should print "true"
    }
}
