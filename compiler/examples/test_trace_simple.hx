package test;

import rayzor.Trace;

class Main {
    static function main() {
        // Test tracing different types
        Trace.traceInt(42);
        Trace.traceFloat(3.14);
        Trace.traceBool(true);
        Trace.traceBool(false);

        // Test with variable
        var x = 100;
        Trace.traceInt(x);

        var y = 2.718;
        Trace.traceFloat(y);
    }
}
