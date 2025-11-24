

use crate::tast::{node::TypedExpression, SourceLocation};

pub fn extract_location_from_expression(expr: &TypedExpression) -> SourceLocation {
    expr.source_location
}
