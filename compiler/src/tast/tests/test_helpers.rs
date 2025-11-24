//! Helper functions for creating test TAST nodes with correct API

use crate::tast::{
    node::{
        TypedExpression, TypedExpressionKind, TypedStatement, TypedFunction,
        LiteralValue, VariableUsage, ExpressionMetadata, Mutability,
        FunctionEffects, TypedParameter, TypedCatchClause, Visibility,
    },
    SymbolId, TypeId, SourceLocation, InternedString, LifetimeId,
};
use std::rc::Rc;
use std::cell::RefCell;

/// Create a test expression with sensible defaults
pub fn create_test_expr(kind: TypedExpressionKind, expr_type: TypeId) -> TypedExpression {
    TypedExpression {
        kind,
        expr_type,
        usage: VariableUsage::Copy,
        lifetime_id: LifetimeId::from_raw(0),
        source_location: SourceLocation::unknown(),
        metadata: ExpressionMetadata::default(),
    }
}

/// Create a null expression
pub fn create_null_expr() -> TypedExpression {
    create_test_expr(TypedExpressionKind::Null, TypeId::from_raw(1))
}

/// Create an integer literal
pub fn create_int_literal(value: i64) -> TypedExpression {
    create_test_expr(
        TypedExpressionKind::Literal {
            value: LiteralValue::Int(value),
        },
        TypeId::from_raw(1), // int type
    )
}

/// Create a string literal
pub fn create_string_literal(value: String) -> TypedExpression {
    create_test_expr(
        TypedExpressionKind::Literal {
            value: LiteralValue::String(value),
        },
        TypeId::from_raw(2), // string type
    )
}

/// Create a variable expression
pub fn create_var_expr(symbol_id: SymbolId, type_id: TypeId) -> TypedExpression {
    create_test_expr(
        TypedExpressionKind::Variable { symbol_id },
        type_id,
    )
}

/// Create a field access expression
pub fn create_field_access(object: TypedExpression, field_symbol: SymbolId, result_type: TypeId) -> TypedExpression {
    create_test_expr(
        TypedExpressionKind::FieldAccess {
            object: Box::new(object),
            field_symbol,
        },
        result_type,
    )
}

/// Create a method call expression
pub fn create_method_call(
    receiver: TypedExpression,
    method_symbol: SymbolId,
    arguments: Vec<TypedExpression>,
    result_type: TypeId,
) -> TypedExpression {
    create_test_expr(
        TypedExpressionKind::MethodCall {
            receiver: Box::new(receiver),
            method_symbol,
            arguments,
            type_arguments: vec![],
        },
        result_type,
    )
}

/// Create a variable declaration statement
pub fn create_var_decl(
    symbol_id: SymbolId,
    var_type: TypeId,
    initializer: Option<TypedExpression>,
    mutable: bool,
) -> TypedStatement {
    TypedStatement::VarDeclaration {
        symbol_id,
        var_type,
        initializer,
        mutability: if mutable { Mutability::Mutable } else { Mutability::Immutable },
        source_location: SourceLocation::unknown(),
    }
}

/// Create an assignment statement
pub fn create_assignment(target: TypedExpression, value: TypedExpression) -> TypedStatement {
    TypedStatement::Assignment {
        target,
        value,
        source_location: SourceLocation::unknown(),
    }
}

/// Create an expression statement
pub fn create_expr_stmt(expr: TypedExpression) -> TypedStatement {
    TypedStatement::Expression {
        expression: expr,
        source_location: SourceLocation::unknown(),
    }
}

/// Create a throw statement
pub fn create_throw(exception: TypedExpression) -> TypedStatement {
    TypedStatement::Throw {
        exception,
        source_location: SourceLocation::unknown(),
    }
}

/// Create a return statement
pub fn create_return(value: Option<TypedExpression>) -> TypedStatement {
    TypedStatement::Return {
        value,
        source_location: SourceLocation::unknown(),
    }
}

/// Create an if statement
pub fn create_if(
    condition: TypedExpression,
    then_branch: TypedStatement,
    else_branch: Option<TypedStatement>,
) -> TypedStatement {
    TypedStatement::If {
        condition,
        then_branch: Box::new(then_branch),
        else_branch: else_branch.map(Box::new),
        source_location: SourceLocation::unknown(),
    }
}

/// Create a block statement
pub fn create_block(statements: Vec<TypedStatement>, scope_id: crate::tast::ScopeId) -> TypedStatement {
    TypedStatement::Block {
        statements,
        scope_id,
        source_location: SourceLocation::unknown(),
    }
}

/// Create a simple test function
pub fn create_test_function(
    name: &str,
    symbol_id: SymbolId,
    parameters: Vec<TypedParameter>,
    return_type: TypeId,
    body: Vec<TypedStatement>,
    interner: &Rc<RefCell<crate::tast::StringInterner>>,
) -> TypedFunction {
    let name_interned = interner.borrow_mut().intern(name);
    
    TypedFunction {
        symbol_id,
        name: name_interned,
        parameters,
        return_type,
        body,
        type_parameters: vec![],
        effects: FunctionEffects::default(),
        source_location: SourceLocation::unknown(),
        visibility: Visibility::Public,
        is_static: false,
        metadata: None,
    }
}

/// Create a simple typed parameter
pub fn create_param(
    name: &str,
    symbol_id: SymbolId,
    param_type: TypeId,
    interner: &Rc<RefCell<crate::tast::StringInterner>>,
) -> TypedParameter {
    let name_interned = interner.borrow_mut().intern(name);
    
    TypedParameter {
        symbol_id,
        name: name_interned,
        param_type,
        is_optional: false,
        default_value: None,
        is_variadic: false,
    }
}