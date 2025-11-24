# Send/Sync Trait Validation at Compile Time

## Overview

When lowering extern calls like `Thread.spawn()` or `Channel.send()`, the compiler must validate that captured types implement the required traits (Send/Sync) **before** allowing the code to compile.

## Validation Points

### 1. Thread.spawn() - Requires Send

```haxe
Thread.spawn(() -> {
    var x = getValue();  // Capture variable x
    doWork(x);
});
```

**Validation:** All captured variables must be `Send`.

### 2. Channel<T> - Requires Send for T

```haxe
var ch = new Channel<Message>(10);
ch.send(msg);  // Message must be Send
```

**Validation:** The channel element type `T` must be `Send`.

### 3. Arc<T> (Shared ownership) - Requires Send + Sync for T

```haxe
var arc = new Arc<Data>();
// Data must be Send (for Arc itself) and Sync (for shared access)
```

**Validation:** The contained type must be both `Send` and `Sync`.

---

## Implementation Strategy

### Phase 1: Trait Storage in TAST

The `TypedClass` already has `derived_traits: Vec<DerivedTrait>` from our earlier work:

```rust
// compiler/src/tast/node.rs
pub struct TypedClass {
    pub name: InternedString,
    pub symbol_id: SymbolId,
    pub type_id: TypeId,
    // ...
    pub derived_traits: Vec<DerivedTrait>,  // ✅ Already exists
}

pub enum DerivedTrait {
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Send,      // ← ADD THIS
    Sync,      // ← ADD THIS
}
```

### Phase 2: Derive Trait Extraction (Already Implemented)

In `ast_lowering.rs`, we already extract `@:derive([Clone, Copy])`. Just add Send/Sync:

```rust
// compiler/src/tast/ast_lowering.rs
fn extract_derived_traits(&self, class_decl: &parser::ClassDecl) -> Vec<DerivedTrait> {
    let trait_names = class_decl.get_derive_traits();
    let mut derived_traits = Vec::new();

    for trait_name in trait_names {
        if let Some(trait_) = DerivedTrait::from_str(&trait_name) {
            derived_traits.push(trait_);
        }
    }

    derived_traits
}
```

### Phase 3: Trait Query System

Add methods to query if a type implements a trait:

```rust
// compiler/src/tast/type_checking.rs (or new file: trait_checker.rs)

pub struct TraitChecker<'a> {
    type_table: &'a RefCell<TypeTable>,
    symbol_table: &'a SymbolTable,
}

impl<'a> TraitChecker<'a> {
    /// Check if a type implements the Send trait
    pub fn is_send(&self, type_id: TypeId) -> bool {
        self.implements_trait(type_id, DerivedTrait::Send)
    }

    /// Check if a type implements the Sync trait
    pub fn is_sync(&self, type_id: TypeId) -> bool {
        self.implements_trait(type_id, DerivedTrait::Sync)
    }

    /// Generic trait checking
    pub fn implements_trait(&self, type_id: TypeId, trait_: DerivedTrait) -> bool {
        let type_table = self.type_table.borrow();
        let type_info = type_table.get(type_id)?;

        match &type_info.kind {
            // Primitives are always Send + Sync
            TypeKind::Int | TypeKind::Float | TypeKind::Bool => true,

            // String is Send but NOT Sync (has interior mutability in our impl)
            TypeKind::String => matches!(trait_, DerivedTrait::Send),

            // Check class for derived traits
            TypeKind::Class { symbol_id, .. } => {
                if let Some(class) = self.find_class(*symbol_id) {
                    self.class_implements_trait(class, trait_)
                } else {
                    false
                }
            }

            // Function types: Send if captures are Send
            TypeKind::Function { .. } => {
                // TODO: Check closure captures
                false
            }

            // Arrays/Vecs: Send if element is Send
            TypeKind::Array { element, .. } => {
                self.implements_trait(*element, trait_)
            }

            // References: &T is Send if T is Sync, &mut T is Send if T is Send
            TypeKind::Reference { inner, is_mutable } => {
                if *is_mutable {
                    // &mut T is Send if T is Send
                    self.implements_trait(*inner, DerivedTrait::Send)
                } else {
                    // &T is Send if T is Sync
                    self.implements_trait(*inner, DerivedTrait::Sync)
                }
            }

            _ => false,
        }
    }

    /// Check if a class implements a trait
    fn class_implements_trait(&self, class: &TypedClass, trait_: DerivedTrait) -> bool {
        // 1. Check if explicitly derived
        if class.derives(trait_) {
            return true;
        }

        // 2. Auto-derive rules (like Rust)
        match trait_ {
            DerivedTrait::Send => {
                // A struct is Send if all fields are Send
                self.auto_derive_send(class)
            }
            DerivedTrait::Sync => {
                // A struct is Sync if all fields are Sync
                self.auto_derive_sync(class)
            }
            _ => false,
        }
    }

    /// Check if all fields are Send (auto-derive)
    fn auto_derive_send(&self, class: &TypedClass) -> bool {
        for field in &class.fields {
            if !self.is_send(field.field_type) {
                return false;
            }
        }
        true
    }

    /// Check if all fields are Sync (auto-derive)
    fn auto_derive_sync(&self, class: &TypedClass) -> bool {
        for field in &class.fields {
            if !self.is_sync(field.field_type) {
                return false;
            }
        }
        true
    }
}
```

### Phase 4: Closure Capture Analysis

When analyzing a closure passed to `Thread.spawn()`, we need to find all captured variables:

```rust
// compiler/src/tast/closure_analysis.rs (new file)

pub struct CaptureAnalyzer<'a> {
    symbol_table: &'a SymbolTable,
    current_scope: ScopeId,
}

#[derive(Debug)]
pub struct CapturedVariable {
    pub symbol_id: SymbolId,
    pub type_id: TypeId,
    pub name: InternedString,
    pub is_mutable: bool,
}

impl<'a> CaptureAnalyzer<'a> {
    /// Analyze a closure to find all captured variables
    pub fn find_captures(&self, closure: &TypedExpression) -> Vec<CapturedVariable> {
        let mut captures = Vec::new();

        // Walk the closure body and find all variable references
        self.walk_expression(closure, &mut captures);

        captures
    }

    fn walk_expression(&self, expr: &TypedExpression, captures: &mut Vec<CapturedVariable>) {
        match &expr.kind {
            TypedExpressionKind::Identifier { symbol, .. } => {
                // Check if this symbol is from an outer scope (captured)
                if self.is_captured(*symbol) {
                    if let Some(sym_info) = self.symbol_table.get_symbol(*symbol) {
                        captures.push(CapturedVariable {
                            symbol_id: *symbol,
                            type_id: sym_info.type_id,
                            name: sym_info.name,
                            is_mutable: sym_info.is_mutable,
                        });
                    }
                }
            }

            // Recursively walk sub-expressions
            TypedExpressionKind::BinaryOp { left, right, .. } => {
                self.walk_expression(left, captures);
                self.walk_expression(right, captures);
            }

            TypedExpressionKind::Call { function, args, .. } => {
                self.walk_expression(function, captures);
                for arg in args {
                    self.walk_expression(arg, captures);
                }
            }

            TypedExpressionKind::Block { statements, .. } => {
                for stmt in statements {
                    self.walk_statement(stmt, captures);
                }
            }

            // ... handle all expression kinds
            _ => {}
        }
    }

    fn is_captured(&self, symbol_id: SymbolId) -> bool {
        // Check if symbol is from outer scope
        // TODO: Implement scope hierarchy checking
        true
    }
}
```

### Phase 5: Validation During Extern Lowering

When lowering `Thread.spawn()`, validate all captures:

```rust
// compiler/src/ir/lowering.rs (or compiler/src/stdlib/thread.rs)

pub fn lower_thread_spawn(
    ctx: &mut LoweringContext,
    closure_expr: &TypedExpression,
) -> Result<IrId, LoweringError> {
    // 1. Analyze closure to find captures
    let capture_analyzer = CaptureAnalyzer::new(ctx.symbol_table, ctx.current_scope);
    let captures = capture_analyzer.find_captures(closure_expr);

    // 2. Validate all captures are Send
    let trait_checker = TraitChecker::new(ctx.type_table, ctx.symbol_table);

    for capture in &captures {
        if !trait_checker.is_send(capture.type_id) {
            // EMIT ERROR
            return Err(LoweringError {
                kind: ErrorKind::SendTraitRequired {
                    variable_name: ctx.string_interner.get(capture.name).unwrap().to_string(),
                    type_name: ctx.type_name(capture.type_id),
                    operation: "Thread.spawn",
                },
                location: closure_expr.source_location,
                message: format!(
                    "Cannot spawn thread: captured variable '{}' of type '{}' does not implement Send",
                    ctx.string_interner.get(capture.name).unwrap(),
                    ctx.type_name(capture.type_id)
                ),
                suggestion: Some(format!(
                    "Add @:derive([Send]) to the type '{}' or don't capture this variable",
                    ctx.type_name(capture.type_id)
                )),
            });
        }
    }

    // 3. If all checks pass, lower to MIR
    let closure_id = ctx.lower_expression(closure_expr)?;
    let thread_handle = ctx.builder.build_intrinsic(
        Intrinsic::ThreadSpawn,
        vec![closure_id]
    );

    Ok(thread_handle)
}
```

### Phase 6: Channel Type Validation

When lowering `new Channel<T>(capacity)`, validate T is Send:

```rust
// compiler/src/stdlib/channel.rs

pub fn lower_channel_new(
    ctx: &mut LoweringContext,
    element_type: TypeId,
    capacity: i32,
) -> Result<IrId, LoweringError> {
    // Validate element type is Send
    let trait_checker = TraitChecker::new(ctx.type_table, ctx.symbol_table);

    if !trait_checker.is_send(element_type) {
        return Err(LoweringError {
            kind: ErrorKind::SendTraitRequired {
                type_name: ctx.type_name(element_type),
                operation: "Channel::new",
            },
            message: format!(
                "Cannot create Channel<{}>: type does not implement Send",
                ctx.type_name(element_type)
            ),
            suggestion: Some(format!(
                "Add @:derive([Send]) to type '{}'",
                ctx.type_name(element_type)
            )),
        });
    }

    // Lower to MIR
    let channel = ctx.builder.build_intrinsic(
        Intrinsic::ChannelNew,
        vec![element_type, capacity]
    );

    Ok(channel)
}
```

---

## Error Messages

### Example 1: Non-Send Capture in Thread.spawn

```haxe
class NotSend {
    var data: String;
}

var x = new NotSend();
Thread.spawn(() -> {
    trace(x);  // Captures x
});
```

**Error:**
```
Error: Cannot spawn thread - captured variable 'x' does not implement Send
  --> example.hx:6:5
   |
6  | Thread.spawn(() -> {
   |               ^^^^^^
   | captured here
   |
note: variable 'x' has type 'NotSend' which does not implement Send
  --> example.hx:5:5
   |
5  | var x = new NotSend();
   |     ^
   |
help: Add @:derive([Send]) to class NotSend:
   |
2  | @:derive([Send])
3  | class NotSend {
```

### Example 2: Non-Send Type in Channel

```haxe
class Message {
    var callback: Void -> Void;  // Function pointers are NOT Send by default
}

var ch = new Channel<Message>(10);
```

**Error:**
```
Error: Cannot create Channel<Message>: type does not implement Send
  --> example.hx:5:10
   |
5  | var ch = new Channel<Message>(10);
   |          ^^^^^^^^^^^^^^^^^^^^^^^^
   |
note: Message does not implement Send because field 'callback' is not Send
  --> example.hx:2:5
   |
2  |     var callback: Void -> Void;
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
help: Either:
  1. Add @:derive([Send]) to Message (if all fields are actually Send), or
  2. Use a different type for inter-thread communication
```

---

## Auto-Derivation Rules (Like Rust)

### Send

A type is automatically Send if:
- All fields are Send
- No explicit `@:derive([!Send])` opt-out

### Sync

A type is automatically Sync if:
- All fields are Sync
- No mutable interior (no `@:interior_mutable` metadata)

### Primitives

| Type    | Send | Sync |
|---------|------|------|
| Int     | ✅   | ✅   |
| Float   | ✅   | ✅   |
| Bool    | ✅   | ✅   |
| String  | ✅   | ❌   |
| Array<T>| T:Send | T:Sync |
| Rc<T>   | ❌   | ❌   |
| Arc<T>  | T:Send+Sync | T:Sync |

---

## Testing Strategy

### Test 1: Primitive Captures (Should Pass)

```haxe
Thread.spawn(() -> {
    var x = 42;      // Int is Send
    var y = "hi";    // String is Send
    trace(x + y);
});
```

✅ **Expected:** Compiles successfully

### Test 2: Non-Send Capture (Should Fail)

```haxe
class NotSend {
    var x: Int;
}

var ns = new NotSend();
Thread.spawn(() -> {
    trace(ns);
});
```

❌ **Expected:** Compile error - "NotSend does not implement Send"

### Test 3: Explicit @:derive([Send])

```haxe
@:derive([Send])
class Message {
    public var data: String;
}

var msg = new Message();
Thread.spawn(() -> {
    trace(msg);
});
```

✅ **Expected:** Compiles successfully

### Test 4: Auto-Derive Send

```haxe
// No explicit @:derive, but all fields are Send
class AutoSend {
    var x: Int;      // Send
    var y: String;   // Send
}

var a = new AutoSend();
Thread.spawn(() -> {
    trace(a);
});
```

✅ **Expected:** Compiles successfully (auto-derived Send)

### Test 5: Channel Type Validation

```haxe
class NotSend {
    var x: Int;
}

var ch = new Channel<NotSend>(10);
```

❌ **Expected:** Compile error - "Channel<NotSend>: NotSend does not implement Send"

---

## Implementation Checklist

**Phase 1: Foundation**
- [ ] Add `Send` and `Sync` to `DerivedTrait` enum
- [ ] Update `DerivedTrait::from_str()` to parse "Send" and "Sync"
- [ ] Test @:derive([Send, Sync]) parsing

**Phase 2: Trait Checker**
- [ ] Create `TraitChecker` struct
- [ ] Implement `is_send()` for all type kinds
- [ ] Implement `is_sync()` for all type kinds
- [ ] Implement auto-derivation rules
- [ ] Add tests for trait checking

**Phase 3: Capture Analysis**
- [ ] Create `CaptureAnalyzer` struct
- [ ] Implement `find_captures()` for closures
- [ ] Walk all expression types
- [ ] Test closure capture detection

**Phase 4: Extern Lowering**
- [ ] Implement `lower_thread_spawn()` with validation
- [ ] Implement `lower_channel_new()` with validation
- [ ] Add proper error messages
- [ ] Test validation errors

**Phase 5: Integration**
- [ ] Integrate TraitChecker into compilation pipeline
- [ ] Add Send/Sync validation to MirSafetyValidator
- [ ] Update error reporting
- [ ] Write comprehensive tests

This gives us Rust-level thread safety guarantees at compile time! 🎯
