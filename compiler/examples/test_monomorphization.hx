// Test monomorphization of generic classes
// This tests the full pipeline: TAST -> HIR -> MIR -> Monomorphization -> Codegen

@:generic
class Container<T> {
    var value: T;

    public function new(v: T) {
        this.value = v;
    }

    public function get(): T {
        return this.value;
    }

    public function set(v: T): Void {
        this.value = v;
    }
}

class Main {
    static function main() {
        // Create Container<Int> - should generate Container__i32
        var intContainer = new Container<Int>(42);
        var intValue = intContainer.get();
        trace("Int container value: " + intValue);

        // Create Container<String> - should generate Container__String
        var strContainer = new Container<String>("hello");
        var strValue = strContainer.get();
        trace("String container value: " + strValue);

        // Verify values
        if (intValue == 42 && strValue == "hello") {
            trace("SUCCESS: Monomorphization works!");
        } else {
            trace("FAILED: Wrong values");
        }
    }
}
