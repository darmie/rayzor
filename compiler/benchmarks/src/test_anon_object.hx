class Main {
    static function main() {
        var p = {x: 10, y: 20};
        trace(p.x);    // 10
        trace(p.y);    // 20

        var q = {a: 3.14, b: 2.71};
        trace(q.a);    // 3.14
        trace(q.b);    // 2.71

        // String field
        var r = {name: "hello", count: 42};
        trace(r.name);   // hello
        trace(r.count);  // 42
    }
}
