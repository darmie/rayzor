/*
 * Test StringTools with 'using' syntax (static extension)
 * This should allow calling StringTools methods as if they were methods on String
 */

using StringTools;

class Main {
    static function main() {
        trace("=== Testing StringTools with 'using' syntax ===");

        // With 'using StringTools', we should be able to call:
        // "hello".startsWith("he") instead of StringTools.startsWith("hello", "he")

        var s = "  hello world  ";

        // Test trim - should work as s.trim() instead of StringTools.trim(s)
        trace("Testing trim...");
        var trimmed = s.trim();
        trace("Trimmed: '" + trimmed + "'");

        if (trimmed == "hello world") {
            trace("PASS: trim works with using syntax");
        } else {
            trace("FAIL: Expected 'hello world'");
        }

        // Test startsWith
        trace("Testing startsWith...");
        var hello = "hello world";
        if (hello.startsWith("hello")) {
            trace("PASS: startsWith works");
        } else {
            trace("FAIL: startsWith should return true");
        }

        // Test endsWith
        trace("Testing endsWith...");
        if (hello.endsWith("world")) {
            trace("PASS: endsWith works");
        } else {
            trace("FAIL: endsWith should return true");
        }

        // Test contains
        trace("Testing contains...");
        if (hello.contains("lo wo")) {
            trace("PASS: contains works");
        } else {
            trace("FAIL: contains should return true");
        }

        // Test replace
        trace("Testing replace...");
        var replaced = hello.replace("world", "rayzor");
        trace("Replaced: '" + replaced + "'");
        if (replaced == "hello rayzor") {
            trace("PASS: replace works");
        } else {
            trace("FAIL: Expected 'hello rayzor'");
        }

        trace("=== Done ===");
    }
}
