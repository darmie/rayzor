/*
 * Test StringBuf pure Haxe implementation
 * Tests string building operations
 */

class Main {
    static function main() {
        trace("=== Testing StringBuf ===");

        // Test 1: Basic add
        trace("--- Test 1: Basic add ---");
        var buf = new StringBuf();
        buf.add("Hello");
        buf.add(" ");
        buf.add("World");
        var result = buf.toString();
        trace("Result: " + result);
        if (result == "Hello World") {
            trace("PASS: Basic add works");
        } else {
            trace("FAIL: Expected 'Hello World'");
        }

        // Test 2: addChar
        trace("--- Test 2: addChar ---");
        var buf2 = new StringBuf();
        buf2.addChar(72);  // 'H'
        buf2.addChar(105); // 'i'
        buf2.addChar(33);  // '!'
        var result2 = buf2.toString();
        trace("Result: " + result2);
        if (result2 == "Hi!") {
            trace("PASS: addChar works");
        } else {
            trace("FAIL: Expected 'Hi!'");
        }

        // Test 3: addSub
        trace("--- Test 3: addSub ---");
        var buf3 = new StringBuf();
        buf3.addSub("Hello World", 0, 5);  // "Hello"
        buf3.add("-");
        buf3.addSub("Hello World", 6, 5);  // "World"
        var result3 = buf3.toString();
        trace("Result: " + result3);
        if (result3 == "Hello-World") {
            trace("PASS: addSub works");
        } else {
            trace("FAIL: Expected 'Hello-World'");
        }

        // Test 4: length property
        trace("--- Test 4: length ---");
        var buf4 = new StringBuf();
        buf4.add("Test");
        if (buf4.length == 4) {
            trace("PASS: length is 4");
        } else {
            trace("FAIL: Expected length 4, got " + buf4.length);
        }

        // Test 5: Multiple operations
        trace("--- Test 5: Multiple operations ---");
        var buf5 = new StringBuf();
        buf5.add("A");
        buf5.addChar(66);  // 'B'
        buf5.add("C");
        buf5.addSub("DEFGH", 0, 2);  // "DE"
        var result5 = buf5.toString();
        trace("Result: " + result5);
        if (result5 == "ABCDE") {
            trace("PASS: Multiple operations work");
        } else {
            trace("FAIL: Expected 'ABCDE'");
        }

        trace("=== StringBuf tests completed! ===");
    }
}
