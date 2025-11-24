//! TAST Node Extensions for Missing Language Features
//!
//! This module adds support for language features that were missing from the
//! original TAST implementation, including:
//! - Do-while loops
//! - Array/Map comprehensions  
//! - Using statements
//! - Metadata/Attributes
//! - Try expressions
//! - Additional expression types

use crate::tast::{InternedString, ScopeId, SourceLocation, SymbolId, TypeId};
use super::node::*;

// ============================================================================
// Extended Statement Types
// ============================================================================

/// Extended statement types to support all Haxe features
#[derive(Debug, Clone)]
pub enum TypedStatementExt {
    /// All existing statements from TypedStatement
    Base(TypedStatement),
    
    /// Do-while loop
    DoWhile {
        body: Box<TypedStatement>,
        condition: TypedExpression,
        source_location: SourceLocation,
    },
    
    /// Using statement (imports static extensions)
    Using {
        module_path: Vec<InternedString>,
        source_location: SourceLocation,
    },
    
    /// Metadata/attribute application
    MetadataApplication {
        metadata: TypedMetadata,
        target: Box<TypedStatement>,
        source_location: SourceLocation,
    },
    
    /// Unsafe block for low-level operations
    Unsafe {
        body: Vec<TypedStatement>,
        scope_id: ScopeId,
        source_location: SourceLocation,
    },
    
    /// Inline assembly or extern code
    InlineCode {
        language: InlineLanguage,
        code: String,
        inputs: Vec<(String, TypedExpression)>,
        outputs: Vec<(String, SymbolId)>,
        source_location: SourceLocation,
    },
}

/// Inline code language types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineLanguage {
    /// C/C++ code
    C,
    /// JavaScript code  
    JavaScript,
    /// Assembly code
    Assembly,
    /// Platform-specific code
    Native,
}

// ============================================================================
// Extended Expression Types
// ============================================================================

/// Extended expression types for missing features
#[derive(Debug, Clone)]
pub enum TypedExpressionExt {
    /// All existing expressions from TypedExpressionKind
    Base(TypedExpressionKind),
    
    /// Try expression (expression form of try-catch)
    TryExpr {
        expr: Box<TypedExpression>,
        catch_clauses: Vec<TypedCatchClause>,
        expr_type: TypeId,
    },
    
    /// Array comprehension: [for (i in 0...10) if (i % 2 == 0) i * 2]
    ArrayComprehension {
        element_expr: Box<TypedExpression>,
        generators: Vec<TypedComprehensionGenerator>,
        guards: Vec<TypedExpression>,
        element_type: TypeId,
    },
    
    /// Map comprehension: [for (k => v in map) if (v > 0) k => v * 2]
    MapComprehension {
        key_expr: Box<TypedExpression>,
        value_expr: Box<TypedExpression>,
        generators: Vec<TypedComprehensionGenerator>,
        guards: Vec<TypedExpression>,
        key_type: TypeId,
        value_type: TypeId,
    },
    
    /// Range expression: 0...10 or 0..10
    Range {
        start: Box<TypedExpression>,
        end: Box<TypedExpression>,
        inclusive: bool,
        range_type: TypeId,
    },
    
    /// Elvis operator: expr ?: default
    Elvis {
        expr: Box<TypedExpression>,
        default: Box<TypedExpression>,
        result_type: TypeId,
    },
    
    /// Null coalescing: expr ?? default
    NullCoalesce {
        expr: Box<TypedExpression>,
        default: Box<TypedExpression>,
        result_type: TypeId,
    },
    
    /// Match expression (expression form of pattern matching)
    MatchExpr {
        value: Box<TypedExpression>,
        arms: Vec<TypedMatchArm>,
        result_type: TypeId,
    },
    
    /// Metadata expression
    MetadataExpr {
        metadata: TypedMetadata,
        expr: Box<TypedExpression>,
    },
    
    /// Type assertion: expr as! Type (unsafe cast)
    TypeAssertion {
        expr: Box<TypedExpression>,
        asserted_type: TypeId,
    },
    
    /// Yield expression (for iterators/generators)
    Yield {
        value: Option<Box<TypedExpression>>,
        yield_type: TypeId,
    },
}

/// Comprehension generator for array/map comprehensions
#[derive(Debug, Clone)]
pub struct TypedComprehensionGenerator {
    /// Variable being bound
    pub variable: SymbolId,
    /// Optional key variable (for map iterations)
    pub key_variable: Option<SymbolId>,
    /// Iterator expression
    pub iterator: TypedExpression,
    /// Source location
    pub source_location: SourceLocation,
}

/// Match arm for match expressions
#[derive(Debug, Clone)]
pub struct TypedMatchArm {
    /// Pattern to match
    pub pattern: TypedPattern,
    /// Guard condition
    pub guard: Option<TypedExpression>,
    /// Result expression
    pub body: TypedExpression,
    /// Variables bound by pattern
    pub bound_variables: Vec<SymbolId>,
    /// Source location
    pub source_location: SourceLocation,
}

// ============================================================================
// Metadata/Attributes
// ============================================================================

/// Typed metadata/attribute
#[derive(Debug, Clone)]
pub struct TypedMetadata {
    /// Metadata name (e.g., ":keep", ":native")
    pub name: InternedString,
    /// Metadata arguments
    pub args: Vec<TypedMetadataArg>,
    /// Source location
    pub source_location: SourceLocation,
}

/// Metadata argument
#[derive(Debug, Clone)]
pub enum TypedMetadataArg {
    /// Literal value
    Literal(LiteralValue),
    /// Named argument
    Named {
        name: InternedString,
        value: Box<TypedMetadataArg>,
    },
    /// Complex expression (for compile-time evaluation)
    Expression(TypedExpression),
}

// ============================================================================
// Extended Type Kinds
// ============================================================================

/// Additional type kinds for complete type system
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeKindExt {
    /// All existing types from TypeKind
    Base(crate::tast::core::TypeKind),
    
    /// Tuple type: (T1, T2, T3)
    Tuple {
        element_types: Vec<TypeId>,
    },
    
    /// Never type (for functions that never return)
    Never,
    
    /// Literal type (const values as types)
    Literal {
        value: LiteralTypeValue,
    },
    
    /// Conditional type: T extends U ? X : Y
    Conditional {
        condition_type: TypeId,
        extends_type: TypeId,
        true_type: TypeId,
        false_type: TypeId,
    },
    
    /// Mapped type (for advanced generics)
    Mapped {
        type_parameter: SymbolId,
        constraint_type: TypeId,
        template_type: TypeId,
    },
    
    /// Infer type (for type inference in generics)
    Infer {
        symbol_id: SymbolId,
    },
}

/// Literal type values
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LiteralTypeValue {
    /// String literal type
    String(String),
    /// Integer literal type
    Int(i64),
    /// Boolean literal type
    Bool(bool),
}

// ============================================================================
// Additional Pattern Types
// ============================================================================

/// Extended pattern types
#[derive(Debug, Clone)]
pub enum TypedPatternExt {
    /// All existing patterns from TypedPattern
    Base(TypedPattern),
    
    /// Tuple pattern: (a, b, c)
    Tuple {
        elements: Vec<TypedPattern>,
        pattern_type: TypeId,
        source_location: SourceLocation,
    },
    
    /// Range pattern: 1..10
    Range {
        start: Box<TypedPattern>,
        end: Box<TypedPattern>,
        inclusive: bool,
        pattern_type: TypeId,
        source_location: SourceLocation,
    },
    
    /// Type pattern: x : Type
    Type {
        pattern: Box<TypedPattern>,
        expected_type: TypeId,
        source_location: SourceLocation,
    },
    
    /// Or pattern: pattern1 | pattern2
    Or {
        patterns: Vec<TypedPattern>,
        pattern_type: TypeId,
        source_location: SourceLocation,
    },
}

// ============================================================================
// Extended Class Features
// ============================================================================

/// Property accessor types
#[derive(Debug, Clone)]
pub struct TypedPropertyAccessor {
    /// Getter implementation
    pub getter: Option<PropertyGetter>,
    /// Setter implementation
    pub setter: Option<PropertySetter>,
}

#[derive(Debug, Clone)]
pub enum PropertyGetter {
    /// Default getter
    Default,
    /// Custom getter function
    Custom(TypedFunction),
    /// Null (no getter)
    Null,
    /// Dynamic getter
    Dynamic,
}

#[derive(Debug, Clone)]
pub enum PropertySetter {
    /// Default setter
    Default,
    /// Custom setter function
    Custom(TypedFunction),
    /// Null (no setter)
    Null,
    /// Never (read-only)
    Never,
}

/// Extended field for properties
#[derive(Debug, Clone)]
pub struct TypedProperty {
    /// Base field information
    pub field: TypedField,
    /// Property accessors
    pub accessors: TypedPropertyAccessor,
    /// Backing field (if any)
    pub backing_field: Option<SymbolId>,
}

// ============================================================================
// Operator Overloading
// ============================================================================

/// Operator overload definition
#[derive(Debug, Clone)]
pub struct TypedOperatorOverload {
    /// Operator being overloaded
    pub operator: OverloadableOperator,
    /// Implementation function
    pub implementation: TypedFunction,
    /// Source location
    pub source_location: SourceLocation,
}

/// Operators that can be overloaded in abstracts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OverloadableOperator {
    // Arithmetic
    Add,        // +
    Subtract,   // -
    Multiply,   // *
    Divide,     // /
    Modulo,     // %
    
    // Comparison
    Equals,     // ==
    NotEquals,  // !=
    Less,       // <
    Greater,    // >
    LessEqual,  // <=
    GreaterEqual, // >=
    
    // Unary
    Negate,     // -x
    Not,        // !x
    
    // Special
    Index,      // []
    IndexSet,   // []= 
    Call,       // ()
}

// ============================================================================
// Helper trait implementations
// ============================================================================

impl HasSourceLocation for TypedStatementExt {
    fn source_location(&self) -> SourceLocation {
        match self {
            TypedStatementExt::Base(stmt) => stmt.source_location(),
            TypedStatementExt::DoWhile { source_location, .. } |
            TypedStatementExt::Using { source_location, .. } |
            TypedStatementExt::MetadataApplication { source_location, .. } |
            TypedStatementExt::Unsafe { source_location, .. } |
            TypedStatementExt::InlineCode { source_location, .. } => *source_location,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_extended_types() {
        // Test that extended types can be created
        let tuple_types = vec![TypeId::from_raw(1), TypeId::from_raw(2)];
        let _tuple = TypeKindExt::Tuple { element_types: tuple_types };
        
        let _never = TypeKindExt::Never;
        
        let _literal = TypeKindExt::Literal {
            value: LiteralTypeValue::String("const".to_string()),
        };
    }
    
    #[test]
    fn test_operator_overloads() {
        let op = OverloadableOperator::Add;
        assert_eq!(op, OverloadableOperator::Add);
        
        let ops = [
            OverloadableOperator::Subtract,
            OverloadableOperator::Multiply,
            OverloadableOperator::Index,
        ];
        
        assert_eq!(ops.len(), 3);
    }
}