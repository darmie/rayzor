/*
 * Test StringTools with direct static method calls (no 'using' syntax)
 */

import StringTools;

class Main {
    static function main() {
        trace("=== Testing StringTools with direct calls ===");

        // First, test lastIndexOf directly on a string
        var hello = "hello world";
        trace("Testing lastIndexOf directly...");
        var lastIdx = hello.lastIndexOf("hello", 0);
        if (lastIdx == 0) {
            trace("lastIndexOf: PASS (got 0)");
        } else {
            trace("lastIndexOf: FAIL");
        }

        // Test string length comparison
        trace("Testing length...");
        var helloLen = hello.length;
        if (helloLen == 11) {
            trace("length: PASS (got 11)");
        } else {
            trace("length: FAIL");
        }

        // Test startsWith via StringTools
        trace("Testing StringTools.startsWith...");
        var startsResult = StringTools.startsWith(hello, "hello");
        if (startsResult) {
            trace("StringTools.startsWith: PASS");
        } else {
            trace("StringTools.startsWith: FAIL");
        }

        trace("=== Done ===");
    }
}
