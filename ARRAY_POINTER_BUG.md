# CRITICAL BUG: Array Pointer Storage/Retrieval

## Issue
Array<T> where T is a pointer type returns elements offset by +32 bytes from stored values.

## Evidence
```
Spawned:  0x157b0f090  (rayzor_thread_spawn returns this)
Retrieved: 0x157b0f0b0  (handles[0] returns this)  
Offset:    +0x20 = 32 bytes
```

## Reproduction
```haxe
var handles = new Array<Thread<Int>>();
var h = Thread.spawn(() -> 42);
handles.push(h);  // Stores 0x157b0f090
var retrieved = handles[0];  // Returns 0x157b0f0b0 ❌
```

## Impact
- Blocks thread_multiple and ALL tests storing pointers in arrays
- Affects both `array[i]` AND `for (x in array)` 
- NOT closure/thread bug - pure Array infrastructure issue

## Root Cause
Array operations map to `haxe_array_*` runtime functions.
Bug in pointer element storage/retrieval (offset calculation).

## Status
- Integer arrays ✅ WORK  
- Pointer arrays ❌ BROKEN (+32 byte offset)

## Next Steps
Debug `haxe_array_push`/`haxe_array_get` runtime implementations.
