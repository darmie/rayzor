package test;

class Main {
    static function main() {
        // Test global trace() with different types
        trace(42);           // Int literal
        trace(3.14159);      // Float literal
        trace(true);         // Bool literal
        trace(false);        // Bool literal

        // Test with variables
        var x = 100;
        trace(x);            // Int variable

        var y = 2.718;
        trace(y);            // Float variable

        var z = true;
        trace(z);            // Bool variable

        // Test with int expression
        trace(5 + 10);       // Int arithmetic
    }
}
