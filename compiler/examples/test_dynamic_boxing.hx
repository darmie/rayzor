package test;

class Main {
    static function main() {
        // Test boxing: concrete values -> Dynamic
        var d1:Dynamic = 42;
        var d2:Dynamic = 3.14;
        var d3:Dynamic = true;

        // Test unboxing: Dynamic -> concrete values
        var i:Int = d1;
        var f:Float = d2;
        var b:Bool = d3;

        // Print results
        trace(i);   // Should print 42
        trace(f);   // Should print 3.14
        trace(b);   // Should print true
    }
}
