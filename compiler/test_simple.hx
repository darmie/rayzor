class SimpleTest {
    public function testComprehension():Array<Int> {
        var items = [1, 2, 3];
        var squares = [for (i in items) i * i];
        return squares;
    }
    
    public function testLogical(a:Bool, b:Bool):Bool {
        return a && b || !a;
    }
}