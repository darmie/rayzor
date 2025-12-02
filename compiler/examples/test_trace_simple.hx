package test;

import rayzor.Trace;

class Main {
    static function main() {
        // Test 1: Explicit Trace methods (should work)
        trace("=== Test 1: Explicit Trace methods ===");
        Trace.traceInt(42);
        Trace.traceFloat(3.14);
        Trace.traceBool(true);
        Trace.traceBool(false);

        // Test 2: Generic trace() with primitives
        trace("=== Test 2: Generic trace() with primitives ===");
        trace("String literal works");
        trace(123);     // Int literal
        trace(3.14);    // Float literal
        trace(true);    // Bool literal - this is what we're testing!
        trace(false);   // Bool literal

        // Test 3: trace() with bool variables
        trace("=== Test 3: Bool variables ===");
        var myTrue = true;
        var myFalse = false;
        trace(myTrue);   // Should print 'true'
        trace(myFalse);  // Should print 'false'

        // Test 4: trace() with comparison results
        trace("=== Test 4: Comparison results ===");
        var cmp1 = 5 > 3;   // true
        var cmp2 = 5 < 3;   // false
        trace(cmp1);
        trace(cmp2);

        trace("=== All tests complete ===");
    }
}
