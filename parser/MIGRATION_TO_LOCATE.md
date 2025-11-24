# Migration Guide: Adding nom_locate Support

This document describes how we're adding nom_locate support to the parser for better position tracking.

## Overview

We've implemented a gradual migration strategy that allows the existing parser to continue working while we add nom_locate support. This approach ensures we don't break any existing functionality while improving position tracking.

## Architecture

### 1. Adapter Layer (`locate_adapter.rs`)
- Provides utilities to convert between `&str` and `LocatedSpan` parsers
- `adapt_str_parser`: Wraps existing `&str` parsers to work with `LocatedSpan`
- `with_span_tracking`: Automatically adds span tracking to any parser
- `EnhancedParserContext`: Context that works with both parser types

### 2. Enhanced Parser (`enhanced_locate_parser.rs`)
- Reimplements key parser functions using `LocatedSpan`
- Reuses existing parser logic through the adapter layer
- Provides proper span tracking for all AST nodes

### 3. Migration Helper (`span_enhanced_parser.rs`)
- Tools to gradually migrate existing parsers
- `SpanEnhancement` trait for adding span tracking
- Helper functions for common patterns

## Usage

### Using the Enhanced Parser

```rust
use parser::{parse_haxe_with_locate, parse_expression_with_locate};

// Parse a complete Haxe file
let ast = parse_haxe_with_locate(source_code)?;

// Parse just an expression
let expr = parse_expression_with_locate("42 + x")?;
```

### Migrating Existing Parser Functions

To migrate an existing parser function to use nom_locate:

1. **Simple Migration** - Use the adapter:
```rust
// Old parser
fn my_parser(input: &str) -> IResult<&str, MyType> {
    // ... parser logic ...
}

// Enhanced version
fn my_parser_enhanced(input: LocSpan) -> LocResult<MyType> {
    adapt_str_parser(my_parser)(input)
}
```

2. **Full Migration** - Rewrite for better span tracking:
```rust
fn my_parser_enhanced(input: LocSpan) -> LocResult<MyType> {
    let start = input.clone();
    
    // ... parser logic using LocSpan ...
    
    let span = make_loc_span(&start, &input);
    Ok((input, MyType { /* fields */, span }))
}
```

## Benefits

1. **Automatic Position Tracking**: LocatedSpan tracks byte positions automatically
2. **Better Error Messages**: Can provide exact line/column information
3. **Incremental Migration**: Can migrate one parser at a time
4. **Backward Compatibility**: Existing code continues to work

## Migration Status

### Completed
- [x] Basic infrastructure (adapter, enhanced parser setup)
- [x] Package declaration
- [x] Import declarations
- [x] Class declarations (basic)
- [x] Type parsing adapter

### In Progress
- [ ] Expression parsing (full migration)
- [ ] Statement parsing
- [ ] Pattern matching

### TODO
- [ ] Complete all declaration types
- [ ] Migrate all expression types
- [ ] Add line/column tracking
- [ ] Enhanced error reporting

## Testing

Run the example to compare both parsers:
```bash
cargo run --example test_locate
```

## Next Steps

1. Continue migrating parser functions incrementally
2. Add comprehensive tests for span accuracy
3. Integrate with error reporting system
4. Eventually phase out the old parser