class TestClass {
    var x:Int = 10;
    var name:String = "test";
    
    public function new() {}
    
    public function test(a:Int):Int {
        var result = a + x;
        
        // Test loop with break/continue
        while (result < 100) {
            if (result > 50) {
                break;
            }
            result = result * 2;
        }
        
        // Test for-in
        var sum = 0;
        for (i in [1, 2, 3]) {
            sum = sum + i;
        }
        
        // Test switch (fixed syntax)
        switch(result) {
            case 42:
                sum = sum + 1;
            case 100:
                sum = sum + 2;
            default:
                sum = sum + 3;
        }
        
        return result + sum;
    }
    
    function testPatterns(value:Dynamic):String {
        // Test pattern matching (fixed syntax)
        return switch(value) {
            case 1 | 2 | 3:
                "small";
            default:
                if (Std.is(value, Int) && value > 100) 
                    "large"
                else 
                    "medium";
        };
    }
    
    static function main() {
        var t = new TestClass();
        trace(t.test(5));
    }
}

enum Color {
    Red;
    Green;
    Blue;
    RGB(r:Int, g:Int, b:Int);
}

interface IDrawable {
    function draw():Void;
}

abstract AbstractInt(Int) from Int to Int {
    public inline function new(i:Int) {
        this = i;
    }
    
    @:op(A + B)
    public inline function add(rhs:AbstractInt):AbstractInt {
        return new AbstractInt(this + rhs.toInt());
    }
    
    public inline function toInt():Int {
        return this;
    }
}