/*
 * Test lastIndexOf directly
 */

class Main {
    static function main() {
        trace("=== Testing lastIndexOf ===");

        var s = "hello";
        trace("s = 'hello'");

        // lastIndexOf("hello", 0) should return 0 since "hello" starts at index 0
        trace("Calling lastIndexOf...");
        var idx = s.lastIndexOf("hello", 0);
        trace("lastIndexOf('hello', 0) returned");

        // Try printing the int directly
        if (idx == 0) {
            trace("PASS: lastIndexOf returned 0");
        } else if (idx == -1) {
            trace("FAIL: lastIndexOf returned -1");
        } else {
            trace("FAIL: lastIndexOf returned unexpected value");
        }

        trace("=== Done ===");
    }
}
