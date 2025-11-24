# Error Code Migration Guide

This document describes the global error numbering system implemented for the Haxe compiler and how to migrate from the old system.

## Overview

The compiler now uses a unified error code system with systematic numbering ranges to prevent conflicts and improve organization.

## Error Code Ranges

| Range | Category | Description |
|-------|----------|-------------|
| E0001-E0999 | Parser | Syntax and parsing errors |
| E1000-E1999 | Type System | Type checking and type-related errors |
| E2000-E2999 | Symbol Resolution | Symbol lookup and scope errors |
| E3000-E3999 | Generics | Generic type and constraint errors |
| E4000-E4999 | Import/Module | Module system and import errors |
| E5000-E5999 | Code Generation | Optimization and code generation errors |
| E6000-E6999 | Metadata | Annotation and metadata errors |
| E7000-E7999 | Macros | Compile-time and macro errors |
| E8000-E8999 | Platform | Target-specific errors |
| E9000-E9999 | Internal | Compiler internal errors |

## Common Error Codes

### Type System (E1000-E1999)
- **E1001**: Type mismatch
- **E1002**: Undefined type
- **E1003**: Invalid type annotation
- **E1004**: Circular type dependency
- **E1005**: Type inference failed
- **E1101**: Function arity mismatch
- **E1102**: Invalid return type
- **E1103**: Parameter type mismatch
- **E1201**: Undefined field
- **E1202**: Field access on non-object
- **E1203**: Field type mismatch
- **E1204**: Private field access

### Symbol Resolution (E2000-E2999)
- **E2001**: Undefined symbol
- **E2002**: Symbol already defined
- **E2003**: Symbol not in scope
- **E2004**: Ambiguous symbol reference
- **E2101**: Private symbol access
- **E2102**: Protected symbol access

### Generics (E3000-E3999)
- **E3001**: Generic parameter count mismatch
- **E3002**: Invalid generic instantiation
- **E3003**: Unconstrained generic parameter
- **E3101**: Constraint violation
- **E3102**: Recursive constraint
- **E3103**: Constraint resolution failed

## Migration Steps

### 1. Update Error Code References

Replace hardcoded error codes with the new system:

```rust
// Old:
.code("E0001")

// New:
.code(format_error_code(1001))  // E1001: Type mismatch
```

### 2. Import the Error Code Module

Add the import to files that generate diagnostics:

```rust
use crate::error_codes::{format_error_code};
```

### 3. Use Appropriate Error Codes

Choose error codes based on the error category:

```rust
match error_kind {
    TypeMismatch => format_error_code(1001),
    UndefinedSymbol => format_error_code(2001),
    ConstraintViolation => format_error_code(3101),
    // etc.
}
```

## Benefits

1. **No Conflicts**: Systematic ranges prevent code collisions
2. **Better Organization**: Clear categorization by compilation phase
3. **Consistency**: All errors follow the same numbering scheme
4. **Documentation**: Rich help text for each error code
5. **Scalability**: Easy to add new codes within ranges
6. **Maintainability**: Centralized registry simplifies updates

## API Reference

### Getting Error Codes

```rust
use crate::error_codes::{error_registry, get_error_code, format_error_code};

// Get error code by number
let error = get_error_code(1001).unwrap();
println!("{}: {}", error.format_code(), error.description);

// Format error code
let code_str = format_error_code(1001); // "E1001"

// Get all type errors
let type_errors = error_registry().get_type_errors();
```

### Adding New Error Codes

Edit `/compiler/src/error_codes.rs` and add to the appropriate range:

```rust
self.register(ErrorCode::new(
    1234,                    // Code number
    "Type",                  // Category
    "Description of error",  // Brief description
    Some("How to fix it")    // Optional help text
));
```

## Testing

Run the error code tests:

```bash
cargo test error_codes
```

This will verify:
- All error codes are properly registered
- Range organization is correct
- Pipeline integration works
- Documentation is complete