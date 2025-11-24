use compiler::ir::lowering::lower_to_hir;
use compiler::tast::{
    node::{TypedFile, TypedFunction, TypedStatement, TypedExpression, TypedExpressionKind, TypedParameter, LiteralValue},
    Type, TypeKind, TypeId, SymbolId, SourceLocation, SymbolTable, TypeTable,
};
use std::rc::Rc;
use std::cell::RefCell;

fn main() {
    println!("Testing IR Lowering...");
    
    // Create symbol and type tables
    let mut symbol_table = SymbolTable::new();
    let type_table = Rc::new(RefCell::new(TypeTable::new()));
    
    // Create a simple typed file with a function
    let mut file = TypedFile {
        module_name: "test".to_string(),
        imports: vec![],
        type_decls: vec![],
        functions: vec![],
        classes: vec![],
        interfaces: vec![],
        enums: vec![],
        typedefs: vec![],
        variables: vec![],
        constants: vec![],
        metadata: vec![],
        source_location: SourceLocation::new(0, 1, 1, 1, 1),
    };
    
    // Add a simple function: function test(): Int { return 42; }
    let func_symbol = SymbolId::from_raw(1);
    let int_type = TypeId::from_raw(1);
    
    // Register the Int type
    type_table.borrow_mut().register(Type {
        kind: TypeKind::Int,
        is_nullable: false,
        source_location: Some(SourceLocation::new(0, 1, 1, 1, 1)),
    });
    
    let function = TypedFunction {
        symbol_id: func_symbol,
        name: "test".into(),
        type_params: vec![],
        parameters: vec![],
        return_type: int_type,
        body: vec![
            TypedStatement::Return {
                value: Some(Box::new(TypedExpression {
                    kind: TypedExpressionKind::Literal {
                        value: LiteralValue::Int(42),
                    },
                    expr_type: int_type,
                    source_location: SourceLocation::new(0, 1, 1, 1, 1),
                })),
                source_location: SourceLocation::new(0, 1, 1, 1, 1),
            }
        ],
        is_static: false,
        is_public: true,
        is_inline: false,
        source_location: SourceLocation::new(0, 1, 1, 1, 1),
    };
    
    file.functions.push(function);
    
    // Lower to HIR
    match lower_to_hir(&file, &symbol_table, &type_table, None, None) {
        Ok(ir_module) => {
            println!("Successfully lowered to HIR!");
            println!("Module name: {}", ir_module.name);
            println!("Functions: {}", ir_module.functions.len());
            
            for (id, func) in &ir_module.functions {
                println!("  Function {}: {}", id.as_raw(), func.name);
                println!("    Return type: {:?}", func.signature.return_type);
                println!("    Basic blocks: {}", func.cfg.blocks.len());
            }
        }
        Err(errors) => {
            println!("Lowering failed with {} errors:", errors.len());
            for error in errors {
                println!("  Error at {}:{}: {}", 
                    error.location.line, 
                    error.location.column, 
                    error.message
                );
            }
        }
    }
}