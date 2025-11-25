package test;

class Main {
    static function main() {
        // Simple test - can we print an integer?
        var x = 42;
        Sys.print("x = ");
        // Try using rayzor.Trace directly
        rayzor.Trace.traceInt(x);
    }
}
