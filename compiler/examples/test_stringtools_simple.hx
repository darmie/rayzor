/*
 * Minimal StringTools test - just isSpace
 */

import StringTools;

class Main {
    static function main() {
        trace("=== Simple StringTools Test ===");

        // Test string length first (known to work)
        trace("Testing string length...");
        var s = " abc";
        var len = s.length;
        trace("length = " + len);

        // Test isSpace directly
        trace("Testing isSpace...");
        var result = StringTools.isSpace(" abc", 0);
        trace("Got isSpace result");

        // Try explicit boolean check - avoid string concat with bool
        if (result) {
            trace("result is true");
            trace("PASS: isSpace detected space");
        } else {
            trace("result is false");
            trace("FAIL: isSpace should detect space");
        }

        trace("=== Done ===");
    }
}
