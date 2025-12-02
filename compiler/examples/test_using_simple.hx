/*
 * Simple test for 'using' static extension
 */

using StringTools;

class Main {
    static function main() {
        var s = "hello";

        // Using static extension: s.startsWith("he") should become StringTools.startsWith(s, "he")
        if (s.startsWith("he")) {
            trace("PASS: startsWith works");
        } else {
            trace("FAIL: startsWith should return true");
        }
    }
}
