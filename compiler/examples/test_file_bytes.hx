/*
 * Test File.getBytes and File.saveBytes
 * Tests binary file I/O using haxe.io.Bytes
 */

import sys.io.File;
import haxe.io.Bytes;

class Main {
    static function main() {
        trace("=== Testing File.getBytes/saveBytes ===");

        // Test 1: Create bytes and save to file
        trace("--- Test 1: saveBytes ---");
        var bytes:Bytes = Bytes.alloc(10);
        bytes.set(0, 72);   // 'H'
        bytes.set(1, 101);  // 'e'
        bytes.set(2, 108);  // 'l'
        bytes.set(3, 108);  // 'l'
        bytes.set(4, 111);  // 'o'
        bytes.set(5, 33);   // '!'
        bytes.set(6, 10);   // newline
        bytes.set(7, 0);
        bytes.set(8, 0);
        bytes.set(9, 0);

        File.saveBytes("/tmp/test_bytes_output.bin", bytes);
        trace("Saved bytes to /tmp/test_bytes_output.bin");

        // Test 2: Read bytes back
        trace("--- Test 2: getBytes ---");
        var readBytes:Bytes = File.getBytes("/tmp/test_bytes_output.bin");
        trace("Read bytes from file");

        // Verify content
        var str = readBytes.toString();
        trace("Content: " + str);

        // Test 3: Save text as bytes and read back
        trace("--- Test 3: ofString/saveBytes/getBytes ---");
        var textBytes:Bytes = Bytes.ofString("Binary file test!");
        File.saveBytes("/tmp/test_text_as_bytes.bin", textBytes);
        trace("Saved text as bytes");

        var readTextBytes:Bytes = File.getBytes("/tmp/test_text_as_bytes.bin");
        trace("Read back: " + readTextBytes.toString());

        trace("=== All File bytes tests completed! ===");
    }
}
