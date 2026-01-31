# CC (TinyCC Runtime) Features

Rayzor embeds TinyCC as a JIT compiler, enabling inline C code execution from Haxe. This document covers all available APIs and metadata.

## Inline C with `__c__()`

The simplest way to execute C code. The compiler auto-manages the TCC lifecycle (create, compile, relocate, call, delete).

```haxe
// Basic: no args, no return
untyped __c__('void __entry__() { printf("hello\\n"); }');

// With args: {0}, {1}, ... are replaced with extern long __argN
var result = untyped __c__('
    long __entry__() { return {0} + {1}; }
', 10, 32);
// result == 42
```

Arguments are passed as `extern long` values. All Haxe reference types (Arc, Vec, Box, class instances) are pointer-sized integers and can be passed directly. Cast to appropriate C types inside the C code.

### System headers

Standard C headers work when platform SDK is installed:

```haxe
untyped __c__('
    #include <string.h>
    long __entry__() { return (long)strlen("hello"); }
');
```

- **macOS**: `xcode-select --install` (CommandLineTools, ~1GB)
- **Linux**: `apt install build-essential` (or equivalent)
- Pure C code (no `#include`) works without any SDK

## Explicit CC API

For more control over the compilation lifecycle:

```haxe
import rayzor.runtime.CC;

var cc = CC.create();
cc.compile('
    #include <math.h>
    double my_sqrt(double x) { return sqrt(x); }
');
cc.relocate();
var fn = cc.getSymbol("my_sqrt");
var result = CC.call1(fn, someValue);
cc.delete();
```

### Methods

| Method | Description |
|--------|-------------|
| `CC.create()` | Create a new TCC context (output to memory) |
| `cc.compile(code)` | Compile C source string |
| `cc.addSymbol(name, value)` | Register a symbol for C `extern long name` access |
| `cc.relocate()` | Link and relocate into executable memory |
| `cc.getSymbol(name)` | Get function/symbol address by name |
| `cc.addFramework(name)` | Load macOS framework or shared library |
| `cc.addIncludePath(path)` | Add include search directory |
| `cc.addFile(path)` | Add `.c`, `.o`, `.a`, `.dylib`/`.so`/`.dll` file |
| `cc.delete()` | Free TCC context (JIT code remains valid) |
| `CC.call0(fn)` | Call JIT function (0 args) |
| `CC.call1(fn, a)` | Call JIT function (1 arg) |
| `CC.call2(fn, a, b)` | Call JIT function (2 args) |
| `CC.call3(fn, a, b, c)` | Call JIT function (3 args) |

## @:cstruct Interop

`@:cstruct` classes generate C-compatible memory layouts accessible from `__c__()`:

```haxe
@:cstruct
class Vec2 {
    public var x:Float = 0.0;
    public var y:Float = 0.0;
}

var v = new Vec2();
v.x = 3.0;
v.y = 4.0;

// Vec2 typedef is auto-injected — no manual cdef() needed
var len = untyped __c__('
    #include <math.h>
    double __entry__() {
        Vec2* v = (Vec2*){0};
        return sqrt(v->x * v->x + v->y * v->y);
    }
', Usize.fromPtr(v));
```

Module-local `@:cstruct` types are auto-injected into `__c__()` contexts at compile time. For imported types from other modules, `cdef()` is available as an explicit opt-in.

## Metadata

Metadata can be applied to classes or functions. When `__c__()` is used, metadata from the enclosing function and all module-local classes is collected automatically.

### @:frameworks

Load macOS frameworks (or shared libraries on Linux) into TCC context:

```haxe
@:frameworks(["Accelerate"])
class Main {
    static function main() {
        untyped __c__('
            #include <Accelerate/Accelerate.h>
            void __entry__() {
                float x[4] = {1,2,3,4};
                float result = cblas_sdot(4, x, 1, x, 1);
            }
        ');
    }
}
```

Or on a function:

```haxe
@:frameworks(["Accelerate"])
static function compute() {
    untyped __c__('...');
}
```

### @:cInclude

Add include search paths:

```haxe
@:cInclude(["/opt/homebrew/include", "/usr/local/include/mylib"])
class Main { ... }
```

### @:cSource

Add C source files to the compilation:

```haxe
@:cSource(["vendor/stb_image.c", "src/helpers.c"])
class Main { ... }
```

### @:clib

Discover and load system libraries via `pkg-config`:

```haxe
@:clib(["sqlite3"])
class Main {
    static function main() {
        untyped __c__('
            #include <sqlite3.h>
            long __entry__() {
                return (long)sqlite3_libversion();
            }
        ');
    }
}
```

This runs `pkg-config --cflags sqlite3` to discover include paths and `pkg-config --libs sqlite3` to load the library. Requires `pkg-config` to be installed:
- **macOS**: `brew install pkg-config`
- **Linux**: `apt install pkg-config`
- **Windows/MSYS2**: `pacman -S pkg-config`

## Error Handling

TCC errors (compilation, relocation, missing symbols) trigger Rust panics, which can be caught with Haxe try-catch:

```haxe
try {
    untyped __c__('this is not valid C');
} catch (e:Dynamic) {
    trace("TCC error: " + e);
}
```

## Platform Notes

- **macOS**: TCC discovers SDK paths automatically via `xcrun --show-sdk-path`. Frameworks are loaded from `/System/Library/Frameworks/` and Homebrew paths.
- **Linux**: Standard paths (`/usr/include`, `/usr/lib`) are used. Install `build-essential` for headers.
- **Windows**: TCC targets MinGW ABI (not MSVC). Best used with MSYS2/MinGW environment. For MSVC-specific libraries, use extern classes as an escape hatch.
