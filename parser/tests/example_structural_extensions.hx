// Example of structural extensions for typedefs in Haxe

// Basic intersection type - extending a type with additional fields
typedef ExtendedPoint = Point & {
    var z:Float;
    var color:String;
};

// Extending an anonymous type
typedef User = {
    var id:Int;
    var name:String;
} & {
    var email:String;
    var ?phone:String;
    function validate():Bool;
};

// Generic typedef with structural extension
typedef Container<T> = Array<T> & {
    var capacity:Int;
    function isFull():Bool;
    function reserve(size:Int):Void;
};

// Multiple intersections (left-associative)
typedef ComplexType = BaseType & {var x:Int;} & {var y:String;} & {var z:Bool;};

// With metadata
@:native("ExtendedNative")
typedef Extended = Native & {
    @:optional var extra:String;
    var required:Int;
};