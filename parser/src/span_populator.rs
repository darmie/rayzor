//! Post-processing module to populate spans in AST after parsing
//! 
//! This is a temporary solution until we properly integrate span tracking
//! into the parser itself.

use crate::ast::*;

/// Populate all spans in a HaxeFile with default values
pub fn populate_spans(file: &mut HaxeFile) {
    let default_span = Span::default();
    file.span = default_span;
    
    if let Some(ref mut pkg) = file.package {
        pkg.span = default_span;
    }
    
    for import in &mut file.imports {
        import.span = default_span;
    }
    
    for decl in &mut file.declarations {
        populate_declaration_spans(decl, default_span);
    }
}

fn populate_declaration_spans(decl: &mut Declaration, span: Span) {
    match decl {
        Declaration::Class(ref mut class) => {
            class.span = span;
            for field in &mut class.fields {
                populate_field_spans(field, span);
            }
        }
        Declaration::Interface(ref mut interface) => {
            interface.span = span;
            for field in &mut interface.fields {
                populate_field_spans(field, span);
            }
        }
        Declaration::Enum(ref mut enum_decl) => {
            enum_decl.span = span;
        }
        Declaration::Typedef(ref mut typedef) => {
            typedef.span = span;
        }
        Declaration::Abstract(ref mut abstract_decl) => {
            abstract_decl.span = span;
            for field in &mut abstract_decl.fields {
                populate_field_spans(field, span);
            }
        }
        Declaration::Function(ref mut func) => {
            func.span = span;
            if let Some(ref mut body) = func.body {
                populate_block_spans(body, span);
            }
        }
    }
}

fn populate_field_spans(field: &mut Field, span: Span) {
    match &mut field.kind {
        FieldKind::Variable { default_value, .. } => {
            if let Some(ref mut value) = default_value {
                populate_expression_spans(value, span);
            }
        }
        FieldKind::Function { body, .. } => {
            if let Some(ref mut body_block) = body {
                populate_block_spans(body_block, span);
            }
        }
        FieldKind::Property { .. } => {
            // Properties don't have expressions in this AST
        }
    }
}

fn populate_block_spans(block: &mut Block, span: Span) {
    for stmt in &mut block.statements {
        populate_statement_spans(stmt, span);
    }
}

fn populate_statement_spans(stmt: &mut Statement, span: Span) {
    match stmt {
        Statement::Expression(ref mut expr, ref mut stmt_span) => {
            *stmt_span = span;
            populate_expression_spans(expr, span);
        }
        Statement::Variable { ref mut value, ref mut span: stmt_span, .. } => {
            *stmt_span = span;
            if let Some(ref mut val) = value {
                populate_expression_spans(val, span);
            }
        }
        Statement::Using { ref mut span: stmt_span, .. } => {
            *stmt_span = span;
        }
    }
}

fn populate_expression_spans(expr: &mut Expression, span: Span) {
    match expr {
        Expression::Literal(_, ref mut expr_span) |
        Expression::Identifier(_, ref mut expr_span) |
        Expression::Null(ref mut expr_span) |
        Expression::This(ref mut expr_span) |
        Expression::Super(ref mut expr_span) => {
            *expr_span = span;
        }
        
        Expression::Binary { ref mut left, ref mut right, ref mut span: expr_span, .. } => {
            *expr_span = span;
            populate_expression_spans(left, span);
            populate_expression_spans(right, span);
        }
        
        Expression::Unary { ref mut operand, ref mut span: expr_span, .. } => {
            *expr_span = span;
            populate_expression_spans(operand, span);
        }
        
        Expression::Assignment { ref mut target, ref mut value, ref mut span: expr_span, .. } => {
            *expr_span = span;
            populate_expression_spans(target, span);
            populate_expression_spans(value, span);
        }
        
        Expression::Call { ref mut function, ref mut args, ref mut span: expr_span, .. } => {
            *expr_span = span;
            populate_expression_spans(function, span);
            for arg in args {
                populate_expression_spans(arg, span);
            }
        }
        
        Expression::FieldAccess { ref mut object, ref mut span: expr_span, .. } => {
            *expr_span = span;
            populate_expression_spans(object, span);
        }
        
        Expression::ArrayAccess { ref mut object, ref mut index, ref mut span: expr_span, .. } => {
            *expr_span = span;
            populate_expression_spans(object, span);
            populate_expression_spans(index, span);
        }
        
        Expression::ArrayLiteral(ref mut elements, ref mut expr_span) => {
            *expr_span = span;
            for elem in elements {
                populate_expression_spans(elem, span);
            }
        }
        
        Expression::MapLiteral(ref mut pairs, ref mut expr_span) => {
            *expr_span = span;
            for (key, value) in pairs {
                populate_expression_spans(key, span);
                populate_expression_spans(value, span);
            }
        }
        
        Expression::ObjectLiteral(ref mut fields, ref mut expr_span) => {
            *expr_span = span;
            for field in fields {
                populate_expression_spans(&mut field.value, span);
            }
        }
        
        Expression::If { ref mut condition, ref mut then_block, ref mut else_block, ref mut span: expr_span } => {
            *expr_span = span;
            populate_expression_spans(condition, span);
            populate_expression_spans(then_block, span);
            if let Some(ref mut else_expr) = else_block {
                populate_expression_spans(else_expr, span);
            }
        }
        
        Expression::Block(ref mut block, ref mut expr_span) => {
            *expr_span = span;
            populate_block_spans(block, span);
        }
        
        Expression::While { ref mut condition, ref mut body, ref mut span: expr_span } => {
            *expr_span = span;
            populate_expression_spans(condition, span);
            populate_expression_spans(body, span);
        }
        
        Expression::DoWhile { ref mut body, ref mut condition, ref mut span: expr_span } => {
            *expr_span = span;
            populate_expression_spans(body, span);
            populate_expression_spans(condition, span);
        }
        
        Expression::For { ref mut iterable, ref mut body, ref mut span: expr_span, .. } => {
            *expr_span = span;
            populate_expression_spans(iterable, span);
            populate_expression_spans(body, span);
        }
        
        Expression::CStyleFor { ref mut init, ref mut condition, ref mut increment, ref mut body, ref mut span: expr_span } => {
            *expr_span = span;
            if let Some(ref mut init_stmt) = init {
                populate_statement_spans(init_stmt, span);
            }
            if let Some(ref mut cond) = condition {
                populate_expression_spans(cond, span);
            }
            if let Some(ref mut incr) = increment {
                populate_expression_spans(incr, span);
            }
            populate_expression_spans(body, span);
        }
        
        Expression::New { ref mut args, ref mut span: expr_span, .. } => {
            *expr_span = span;
            for arg in args {
                populate_expression_spans(arg, span);
            }
        }
        
        Expression::Cast { ref mut expr, ref mut span: expr_span, .. } => {
            *expr_span = span;
            populate_expression_spans(expr, span);
        }
        
        Expression::ArrowFunction { ref mut body, ref mut span: expr_span, .. } => {
            *expr_span = span;
            populate_expression_spans(body, span);
        }
        
        Expression::ArrayComprehension { ref mut body, ref mut filter, ref mut span: expr_span, ref mut generators } => {
            *expr_span = span;
            populate_expression_spans(body, span);
            if let Some(ref mut filter_expr) = filter {
                populate_expression_spans(filter_expr, span);
            }
            for gen in generators {
                populate_expression_spans(&mut gen.iterable, span);
            }
        }
        
        Expression::MapComprehension { ref mut key, ref mut value, ref mut filter, ref mut span: expr_span, ref mut generators } => {
            *expr_span = span;
            populate_expression_spans(key, span);
            populate_expression_spans(value, span);
            if let Some(ref mut filter_expr) = filter {
                populate_expression_spans(filter_expr, span);
            }
            for gen in generators {
                populate_expression_spans(&mut gen.iterable, span);
            }
        }
        
        _ => {} // Handle any remaining cases
    }
}