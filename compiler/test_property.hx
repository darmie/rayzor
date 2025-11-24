class TestProperty {
    private var _x:Int;

    public var x(get, set):Int;

    function get_x():Int {
        return _x * 2;
    }

    function set_x(value:Int):Int {
        _x = value;
        return value;
    }

    public function new() {
        _x = 5;
    }

    static function main() {
        var obj = new TestProperty();
        var value = obj.x;  // Should call get_x(), return 10
        trace(value);

        obj.x = 15;  // Should call set_x(15), setting _x to 15
        var value2 = obj.x;  // Should call get_x(), return 30
        trace(value2);
    }
}
