# Improved Generic Error Messages Design

## Current Issues

1. **Wrong Location**: Errors point to the class definition constraint instead of the instantiation site
2. **Confusing Message**: "expected 0, got 1" doesn't clearly explain what's wrong
3. **Missing Context**: Doesn't explain which constraint failed or why

## Proposed Improvements

### 1. Better Error Location Tracking

The error should point to where the generic type is being instantiated, not where it's defined:

```
error[E3101]: Type constraint violation
  --> generic_test.hx:14:29
  |
14 |         var container = new Container<String>();
  |                             ^^^^^^^^^^^^^^^^^ String does not implement Sortable<String>
```

### 2. Clearer Error Messages

Instead of "Type argument count mismatch: expected 0, got 1", provide context:

```
error[E3101]: Type constraint violation
  --> generic_test.hx:14:29
  |
14 |         var container = new Container<String>();
  |                             ^^^^^^^^^^^^^^^^^ 
  |
  = note: Container requires type parameter T to implement Sortable<T>
  = note: String does not implement Sortable<String>
  = help: Use a type that implements Sortable or add the implementation
```

### 3. Show the Constraint Definition

Include a secondary label showing where the constraint is defined:

```
error[E3101]: Type constraint violation
  --> generic_test.hx:14:29
  |
14 |         var container = new Container<String>();
  |                             ^^^^^^^^^^^^^^^^^ String does not implement Sortable<String>
  |
  ::: generic_test.hx:4:19
  |
4  | class Container<T:Sortable<T>> {
  |                   ----------- constraint defined here
```

## Implementation Plan

1. **Track instantiation context**: When processing `new` expressions, store the location
2. **Improve error creation**: Pass both definition and usage locations to error handlers
3. **Enhanced error formatting**: Update the error formatter to show both locations
4. **Better constraint checking**: Provide detailed reasons for constraint failures