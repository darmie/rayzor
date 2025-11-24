//! AST to TAST Lowering
//!
//! This module converts the parser's AST representation into the compiler's
//! Typed Abstract Syntax Tree (TAST) representation, handling:
//! - Symbol resolution and creation
//! - Type annotation processing
//! - Scope management
//! - Error collection and reporting

use crate::tast::{
    core::*,
    node::*,
    *,
};
use crate::tast::node::HasSourceLocation;
use parser::{
    HaxeFile, TypeDeclaration, ClassDecl, InterfaceDecl, EnumDecl, AbstractDecl, TypedefDecl,
    ClassField, ClassFieldKind, Import, Using, Type, Modifier, TypeParam, EnumConstructor,
    BinaryOp, UnaryOp, Expr, ExprKind, FunctionParam, Function, Package, ModuleField
};
use std::collections::HashMap;
use std::cell::RefCell;
use std::rc::Rc;

/// Errors that can occur during AST lowering
#[derive(Debug, Clone)]
pub enum LoweringError {
    /// Symbol resolution failed
    UnresolvedSymbol {
        name: String,
        location: SourceLocation,
    },
    /// Type resolution failed
    UnresolvedType {
        type_name: String,
        location: SourceLocation,
    },
    /// Duplicate symbol definition
    DuplicateSymbol {
        name: String,
        original_location: SourceLocation,
        duplicate_location: SourceLocation,
    },
    /// Invalid modifier combination
    InvalidModifiers {
        modifiers: Vec<String>,
        location: SourceLocation,
    },
    /// Generic type parameter error
    GenericParameterError {
        message: String,
        location: SourceLocation,
    },
    /// Internal lowering error
    InternalError {
        message: String,
        location: SourceLocation,
    },
}

/// Result type for lowering operations
pub type LoweringResult<T> = Result<T, LoweringError>;

/// Typed declaration wrapper for lowering
#[derive(Debug, Clone)]
pub enum TypedDeclaration {
    Function(TypedFunction),
    Class(TypedClass),
    Interface(TypedInterface),
    Enum(TypedEnum),
    TypeAlias(TypedTypeAlias),
    Abstract(TypedAbstract),
}



/// Typed typedef declaration for lowering
#[derive(Debug, Clone)]
pub struct TypedTypedef {
    pub symbol_id: SymbolId,
    pub name: String,
    pub target_type: TypeId,
    pub type_parameters: Vec<TypedTypeParameter>,
    pub visibility: Visibility,
    pub source_location: SourceLocation,
}

/// Context for AST lowering operations
pub struct LoweringContext<'a> {
    pub string_interner: &'a mut StringInterner,
    pub symbol_table: &'a mut SymbolTable,
    pub type_table: &'a RefCell<TypeTable>,
    pub scope_tree: &'a mut ScopeTree,
    pub current_scope: ScopeId,
    pub errors: Vec<LoweringError>,
    pub type_parameter_stack: Vec<HashMap<String, TypeId>>,
    pub span_converter: Option<super::span_conversion::SpanConverter>,
}

impl<'a> LoweringContext<'a> {
    pub fn new(
        string_interner: &'a mut StringInterner,
        symbol_table: &'a mut SymbolTable,
        type_table: &'a RefCell<TypeTable>,
        scope_tree: &'a mut ScopeTree,
        current_scope: ScopeId,
    ) -> Self {
        Self {
            string_interner,
            symbol_table,
            type_table,
            scope_tree,
            current_scope,
            errors: Vec::new(),
            type_parameter_stack: Vec::new(),
            span_converter: None,
        }
    }

    /// Add an error to the context
    pub fn add_error(&mut self, error: LoweringError) {
        self.errors.push(error);
    }

    /// Enter a new scope
    pub fn enter_scope(&mut self, _scope_kind: ScopeKind) -> ScopeId {
        let new_scope = self.scope_tree.create_scope(Some(self.current_scope));
        self.current_scope = new_scope;
        new_scope
    }

    /// Exit the current scope
    pub fn exit_scope(&mut self) {
        if let Some(scope) = self.scope_tree.get_scope(self.current_scope) {
            if let Some(parent) = scope.parent_id {
                self.current_scope = parent;
            }
        }
    }

    /// Push type parameters onto the stack
    pub fn push_type_parameters(&mut self, type_params: HashMap<String, TypeId>) {
        self.type_parameter_stack.push(type_params);
    }

    /// Pop type parameters from the stack
    pub fn pop_type_parameters(&mut self) {
        self.type_parameter_stack.pop();
    }

    /// Resolve a type parameter
    pub fn resolve_type_parameter(&self, name: &str) -> Option<TypeId> {
        for scope in self.type_parameter_stack.iter().rev() {
            if let Some(&type_id) = scope.get(name) {
                return Some(type_id);
            }
        }
        None
    }

    /// Intern a string
    pub fn intern_string(&mut self, s: &str) -> InternedString {
        self.string_interner.intern(s)
    }

    /// Create a source location (simplified)
    pub fn create_location(&self) -> SourceLocation {
        SourceLocation::unknown()
    }

    /// Generate next scope ID for new scopes
    pub fn next_scope_id(&mut self) -> u32 {
        // Create a new scope and return its raw ID
        let scope = self.scope_tree.create_scope(Some(self.current_scope));
        scope.as_raw()
    }
}

/// Main AST lowering implementation
pub struct AstLowering<'a> {
    context: LoweringContext<'a>,
}

impl<'a> AstLowering<'a> {
    pub fn new(
        string_interner: &'a mut StringInterner,
        symbol_table: &'a mut SymbolTable,
        type_table: &'a RefCell<TypeTable>,
        scope_tree: &'a mut ScopeTree,
    ) -> Self {
        let root_scope = ScopeId::first(); // Use first scope as root
        let context = LoweringContext::new(
            string_interner,
            symbol_table,
            type_table,
            scope_tree,
            root_scope,
        );
        
        Self { context }
    }

    /// Lower a complete Haxe file to TAST
    pub fn lower_file(&mut self, file: &HaxeFile) -> LoweringResult<TypedFile> {
        let mut typed_file = TypedFile::new(Rc::new(RefCell::new(StringInterner::new())));

        // Process package declaration
        if let Some(package) = &file.package {
            typed_file.metadata.package_name = Some(package.path.join("."));
        }

        // Process imports
        for import in &file.imports {
            typed_file.imports.push(self.lower_import(import)?);
        }

        // Process using statements
        for using in &file.using {
            typed_file.using_statements.push(self.lower_using(using)?);
        }

        // Process module-level fields
        for module_field in &file.module_fields {
            typed_file.module_fields.push(self.lower_module_field(module_field)?);
        }

        // Process declarations
        for declaration in &file.declarations {
            match self.lower_declaration(declaration) {
                Ok(typed_decl) => {
                    match typed_decl {
                        TypedDeclaration::Function(func) => typed_file.functions.push(func),
                        TypedDeclaration::Class(class) => typed_file.classes.push(class),
                        TypedDeclaration::Interface(interface) => typed_file.interfaces.push(interface),
                        TypedDeclaration::Enum(enum_decl) => typed_file.enums.push(enum_decl),
                        TypedDeclaration::TypeAlias(alias) => typed_file.type_aliases.push(alias),
                        TypedDeclaration::Abstract(abstract_decl) => {
                            typed_file.abstracts.push(abstract_decl);
                        }
                    }
                }
                Err(e) => self.context.add_error(e),
            }
        }

        // Check for errors
        if !self.context.errors.is_empty() {
            return Err(self.context.errors.clone().into_iter().next().unwrap());
        }

        Ok(typed_file)
    }

    /// Extract module name from file
    fn extract_module_name(&self, file: &HaxeFile) -> String {
        if let Some(package) = &file.package {
            package.path.join(".")
        } else {
            "default".to_string()
        }
    }

    /// Lower an import declaration
    fn lower_import(&mut self, import: &Import) -> LoweringResult<TypedImport> {
        let imported_symbols = match &import.mode {
            parser::ImportMode::Normal => Some(vec![import.path.last().unwrap_or(&"".to_string()).clone()]),
            parser::ImportMode::Alias(alias) => Some(vec![alias.clone()]),
            parser::ImportMode::Field(field) => Some(vec![field.clone()]),
            parser::ImportMode::Wildcard => None,
            parser::ImportMode::WildcardWithExclusions(_) => None,
        };
        
        let alias = match &import.mode {
            parser::ImportMode::Alias(alias) => Some(alias.clone()),
            _ => None,
        };
        
        Ok(TypedImport {
            module_path: import.path.join("."),
            imported_symbols,
            alias,
            source_location: self.context.create_location(),
        })
    }
    
    /// Lower a using declaration
    fn lower_using(&mut self, using: &Using) -> LoweringResult<TypedUsing> {
        Ok(TypedUsing {
            module_path: using.path.join("."),
            target_type: None, // TODO: Handle target type if specified
            source_location: self.context.create_location(),
        })
    }
    
    /// Lower a module field
    fn lower_module_field(&mut self, module_field: &ModuleField) -> LoweringResult<TypedModuleField> {
        let field_name = match &module_field.kind {
            parser::ModuleFieldKind::Var { name, .. } => name.clone(),
            parser::ModuleFieldKind::Final { name, .. } => name.clone(),
            parser::ModuleFieldKind::Function(func) => func.name.clone(),
        };
        
        let interned_name = self.context.intern_string(&field_name);
        let field_symbol = self.context.symbol_table.create_variable(interned_name);
        
        let kind = match &module_field.kind {
            parser::ModuleFieldKind::Var { name: _, type_hint, expr } => {
                let field_type = if let Some(type_hint) = type_hint {
                    self.lower_type(type_hint)?
                } else {
                    self.context.type_table.borrow().dynamic_type()
                };
                
                let initializer = if let Some(expr) = expr {
                    Some(self.lower_expression(expr)?)
                } else {
                    None
                };
                
                TypedModuleFieldKind::Var {
                    field_type,
                    initializer,
                    mutability: crate::tast::Mutability::Mutable,
                }
            }
            parser::ModuleFieldKind::Final { name: _, type_hint, expr } => {
                let field_type = if let Some(type_hint) = type_hint {
                    self.lower_type(type_hint)?
                } else {
                    self.context.type_table.borrow().dynamic_type()
                };
                
                let initializer = if let Some(expr) = expr {
                    Some(self.lower_expression(expr)?)
                } else {
                    None
                };
                
                TypedModuleFieldKind::Final {
                    field_type,
                    initializer,
                }
            }
            parser::ModuleFieldKind::Function(func) => {
                TypedModuleFieldKind::Function(self.lower_function_object(func)?)
            }
        };
        
        Ok(TypedModuleField {
            symbol_id: field_symbol,
            name: interned_name,
            kind,
            visibility: Visibility::Public, // TODO: Handle visibility from access modifier
            source_location: self.context.create_location(),
        })
    }

    /// Lower a declaration
    fn lower_declaration(&mut self, declaration: &TypeDeclaration) -> LoweringResult<TypedDeclaration> {
        match declaration {
            TypeDeclaration::Class(class_decl) => {
                self.lower_class_declaration(class_decl)
            }
            TypeDeclaration::Interface(interface_decl) => {
                self.lower_interface_declaration(interface_decl)
            }
            TypeDeclaration::Enum(enum_decl) => {
                self.lower_enum_declaration(enum_decl)
            }
            TypeDeclaration::Typedef(typedef_decl) => {
                self.lower_typedef_declaration(typedef_decl)
            }
            TypeDeclaration::Abstract(abstract_decl) => {
                self.lower_abstract_declaration(abstract_decl)
            }
            TypeDeclaration::Conditional(_) => {
                // For now, skip conditional compilation blocks
                // TODO: Handle conditional compilation properly
                Ok(TypedDeclaration::Function(TypedFunction {
                    symbol_id: SymbolId::invalid(),
                    name: self.context.intern_string("__conditional_placeholder"),
                    parameters: Vec::new(),
                    return_type: self.context.type_table.borrow().void_type(),
                    body: Vec::new(),
                    visibility: Visibility::Public,
                    effects: crate::tast::node::FunctionEffects::default(),
                    type_parameters: Vec::new(),
                    source_location: self.context.create_location(),
                    metadata: FunctionMetadata::default(),
                }))
            }
        }
    }

    /// Lower a class declaration
    fn lower_class_declaration(&mut self, class_decl: &ClassDecl) -> LoweringResult<TypedDeclaration> {
        let class_name = self.context.intern_string(&class_decl.name);
        let class_symbol = self.context.symbol_table.create_class(class_name);

        // Enter class scope
        let class_scope = self.context.enter_scope(ScopeKind::Class);

        // Process type parameters
        let type_params = self.lower_type_parameters(&class_decl.type_params)?;
        let type_param_map: HashMap<String, TypeId> = type_params.iter()
            .map(|tp| (tp.name.clone(), TypeId::invalid())) // Use invalid type for now
            .collect();
        self.context.push_type_parameters(type_param_map);

        // Process extends clause
        let extends = if let Some(extends_type) = &class_decl.extends {
            Some(self.lower_type(extends_type)?)
        } else {
            None
        };

        // Process implements clause
        let implements = class_decl.implements.iter()
            .map(|t| self.lower_type(t))
            .collect::<Result<Vec<_>, _>>()?;

        // Process fields
        let mut fields = Vec::new();
        for field in &class_decl.fields {
            match self.lower_field(field) {
                Ok(typed_field) => fields.push(typed_field),
                Err(e) => self.context.add_error(e),
            }
        }

        // Process modifiers
        let modifiers = self.lower_modifiers(&class_decl.modifiers)?;

        self.context.pop_type_parameters();
        self.context.exit_scope();

        let typed_class = TypedClass {
            symbol_id: class_symbol,
            name: class_name,
            super_class: extends,
            interfaces: implements,
            fields: Vec::new(), // TODO: Convert field types
            methods: Vec::new(), // TODO: Extract methods from fields
            constructors: Vec::new(), // TODO: Extract constructors
            type_parameters: Vec::new(), // TODO: Convert type parameters
            visibility: Visibility::Public,
            source_location: self.context.create_location(),
        };

        Ok(TypedDeclaration::Class(typed_class))
    }

    /// Lower an interface declaration
    fn lower_interface_declaration(&mut self, interface_decl: &InterfaceDecl) -> LoweringResult<TypedDeclaration> {
        let interface_name = self.context.intern_string(&interface_decl.name);
        let interface_symbol = self.context.symbol_table.create_interfce(interface_name);

        // Enter interface scope
        let interface_scope = self.context.enter_scope(ScopeKind::Interface);

        // Process type parameters
        let type_params = self.lower_type_parameters(&interface_decl.type_params)?;
        let type_param_map: HashMap<String, TypeId> = type_params.iter()
            .map(|tp| (tp.name.clone(), TypeId::invalid())) // Use invalid type for now
            .collect();
        self.context.push_type_parameters(type_param_map);

        // Process extends clause
        let extends = interface_decl.extends.iter()
            .map(|t| self.lower_type(t))
            .collect::<Result<Vec<_>, _>>()?;

        // Process fields
        let mut fields = Vec::new();
        for field in &interface_decl.fields {
            match self.lower_field(field) {
                Ok(typed_field) => fields.push(typed_field),
                Err(e) => self.context.add_error(e),
            }
        }

        // Process modifiers
        let modifiers = self.lower_modifiers(&interface_decl.modifiers)?;

        self.context.pop_type_parameters();
        self.context.exit_scope();

        let typed_interface = TypedInterface {
            symbol_id: interface_symbol,
            name: interface_name.to_string(),
            extends,
            methods: Vec::new(), // TODO: Convert fields to method signatures
            type_parameters: Vec::new(), // TODO: Convert type parameters
            visibility: Visibility::Public,
            source_location: self.context.create_location(),
        };

        Ok(TypedDeclaration::Interface(typed_interface))
    }

    /// Lower an enum declaration
    fn lower_enum_declaration(&mut self, enum_decl: &EnumDecl) -> LoweringResult<TypedDeclaration> {
        let enum_name = self.context.intern_string(&enum_decl.name);
        // For now, use a generic symbol creation approach
        let enum_symbol = self.context.symbol_table.create_enum(enum_name);

        // Enter enum scope
        let enum_scope = self.context.enter_scope(ScopeKind::Enum);

        // Process type parameters
        let type_params = self.lower_type_parameters(&enum_decl.type_params)?;
        let type_param_map: HashMap<String, TypeId> = type_params.iter()
            .map(|tp| (tp.name.clone(), TypeId::invalid())) // Use invalid type for now
            .collect();
        self.context.push_type_parameters(type_param_map);

        // Process variants
        let mut variants = Vec::new();
        for variant in &enum_decl.constructors {
            variants.push(self.lower_enum_variant(variant)?);
        }

        // Process modifiers - skip for now since EnumDecl doesn't have modifiers in new parser
        // let modifiers = self.lower_modifiers(&enum_decl.modifiers)?;

        self.context.pop_type_parameters();
        self.context.exit_scope();

        let typed_enum = TypedEnum {
            symbol_id: enum_symbol,
            name: enum_name.to_string(),
            variants,
            type_parameters: Vec::new(), // TODO: Convert type parameters
            visibility: Visibility::Public,
            source_location: self.context.create_location(),
        };

        Ok(TypedDeclaration::Enum(typed_enum))
    }

    /// Lower a typedef declaration
    fn lower_typedef_declaration(&mut self, typedef_decl: &TypedefDecl) -> LoweringResult<TypedDeclaration> {
        let typedef_name = self.context.intern_string(&typedef_decl.name);
        let typedef_symbol = self.context.symbol_table.create_class(typedef_name); // Reuse class creation

        // Process target type
        let target_type = self.lower_type(&typedef_decl.type_def)?;

        let typed_typedef = TypedTypeAlias {
            symbol_id: typedef_symbol,
            name: typedef_name.to_string(),
            target_type,
            type_parameters: Vec::new(),
            visibility: Visibility::Public,
            source_location: self.context.create_location(),
        };

        Ok(TypedDeclaration::TypeAlias(typed_typedef))
    }

    /// Lower an abstract declaration
    fn lower_abstract_declaration(&mut self, abstract_decl: &AbstractDecl) -> LoweringResult<TypedDeclaration> {
        let abstract_name = self.context.intern_string(&abstract_decl.name);
        let abstract_symbol = self.context.symbol_table.create_class(abstract_name);

        // Process underlying type
        let underlying_type = Some(self.lower_type(&abstract_decl.underlying)?);

        let typed_abstract = crate::tast::node::TypedAbstract {
            symbol_id: abstract_symbol,
            name: abstract_name,
            underlying_type,
            type_parameters: Vec::new(), // TODO: Convert type parameters
            fields: Vec::new(), // TODO: Convert fields
            methods: Vec::new(), // TODO: Convert methods
            constructors: Vec::new(), // TODO: Convert constructors
            from_types: Vec::new(), // TODO: Convert from types
            to_types: Vec::new(), // TODO: Convert to types
            visibility: Visibility::Public,
            source_location: self.context.create_location(),
        };

        Ok(TypedDeclaration::Abstract(typed_abstract))
    }

    /// Lower a function declaration (not used anymore - functions are in module fields)
    fn lower_function_declaration(&mut self, function_decl: &Function) -> LoweringResult<TypedDeclaration> {
        let function_name = self.context.intern_string(&function_decl.name);
        let function_symbol = self.context.symbol_table.create_function(function_name);

        // Enter function scope
        let function_scope = self.context.enter_scope(ScopeKind::Function);

        // Process type parameters
        let type_params = self.lower_type_parameters(&function_decl.type_params)?;
        let type_param_map: HashMap<String, TypeId> = type_params.iter()
            .map(|tp| (tp.name.clone(), TypeId::invalid())) // Use invalid type for now
            .collect();
        self.context.push_type_parameters(type_param_map);

        // Process parameters
        let mut parameters = Vec::new();
        for param in &function_decl.params {
            parameters.push(self.lower_parameter(param)?);
        }

        // Process return type
        let return_type = if let Some(ret_type) = &function_decl.return_type {
            self.lower_type(ret_type)?
        } else {
            self.context.type_table.borrow().void_type()
        };

        // Process body
        let body = if let Some(body_expr) = &function_decl.body {
            // Convert expression to statement
            vec![self.lower_expression_as_statement(body_expr)?]
        } else {
            Vec::new()
        };

        // Process modifiers - skip for now

        self.context.pop_type_parameters();
        self.context.exit_scope();

        let typed_function = TypedFunction {
            symbol_id: function_symbol,
            name: function_name,
            parameters,
            return_type,
            body,
            visibility: Visibility::Public,
            effects: crate::tast::node::FunctionEffects::default(),
            type_parameters: Vec::new(), // TODO: Convert type parameters
            source_location: self.context.create_location(),
            metadata: FunctionMetadata::default(),
        };

        Ok(TypedDeclaration::Function(typed_function))
    }

    /// Lower type parameters
    fn lower_type_parameters(&mut self, type_params: &[TypeParam]) -> LoweringResult<Vec<TypedTypeParameter>> {
        let mut result = Vec::new();
        for type_param in type_params {
            result.push(self.lower_type_parameter(type_param)?);
        }
        Ok(result)
    }

    /// Lower a single type parameter
    fn lower_type_parameter(&mut self, type_param: &TypeParam) -> LoweringResult<TypedTypeParameter> {
        let name = self.context.intern_string(&type_param.name);
        
        // Process constraints
        let constraints = type_param.constraints.iter()
            .map(|c| self.lower_type(c))
            .collect::<Result<Vec<_>, _>>()?;

        // Create type parameter symbol
        let symbol_id = self.context.symbol_table.create_type_parameter(name, Vec::new());

        Ok(TypedTypeParameter {
            symbol_id,
            name: name.to_string(),
            constraints,
            variance: TypeVariance::Invariant, // Default variance
            source_location: self.context.create_location(),
        })
    }

    /// Lower a field
    fn lower_field(&mut self, field: &ClassField) -> LoweringResult<TypedField> {
        // For now, create a simple field structure
        let field_name = match &field.kind {
            ClassFieldKind::Var { name, .. } => name.clone(),
            ClassFieldKind::Final { name, .. } => name.clone(),
            ClassFieldKind::Property { name, .. } => name.clone(),
            ClassFieldKind::Function(func) => func.name.clone(),
        };

        let interned_field_name = self.context.intern_string(&field_name);
        let field_symbol = self.context.symbol_table.create_variable(
            interned_field_name
        );

        let field_type = self.context.type_table.borrow().dynamic_type();

        Ok(TypedField {
            symbol_id: field_symbol,
            name: field_name,
            field_type,
            initializer: None,
            mutability: crate::tast::Mutability::Mutable,
            visibility: Visibility::Public,
            is_static: false,
            source_location: self.context.create_location(),
        })
    }

    /// Lower an enum variant
    fn lower_enum_variant(&mut self, variant: &EnumConstructor) -> LoweringResult<TypedEnumVariant> {
        let variant_name = self.context.intern_string(&variant.name);
        let variant_symbol = self.context.symbol_table.create_enum(variant_name);

        // Process parameters
        let mut parameters = Vec::new();
        for param in &variant.params {
            parameters.push(self.lower_parameter(param)?);
        }

        Ok(TypedEnumVariant {
            name: variant_name.to_string(),
            parameters,
            source_location: self.context.create_location(),
        })
    }
    
    /// Lower a function object
    fn lower_function_object(&mut self, func: &Function) -> LoweringResult<TypedFunction> {
        let function_name = self.context.intern_string(&func.name);
        let function_symbol = self.context.symbol_table.create_function(function_name);

        // Enter function scope
        let function_scope = self.context.enter_scope(ScopeKind::Function);

        // Process type parameters
        let type_params = self.lower_type_parameters(&func.type_params)?;
        let type_param_map: HashMap<String, TypeId> = type_params.iter()
            .map(|tp| (tp.name.clone(), TypeId::invalid())) // Use invalid type for now
            .collect();
        self.context.push_type_parameters(type_param_map);

        // Process parameters
        let mut parameters = Vec::new();
        for param in &func.params {
            parameters.push(self.lower_parameter(param)?);
        }

        // Process return type
        let return_type = if let Some(ret_type) = &func.return_type {
            self.lower_type(ret_type)?
        } else {
            self.context.type_table.borrow().void_type()
        };

        // Process body
        let body = if let Some(body_expr) = &func.body {
            vec![self.lower_expression_as_statement(body_expr)?]
        } else {
            Vec::new()
        };

        self.context.pop_type_parameters();
        self.context.exit_scope();

        Ok(TypedFunction {
            symbol_id: function_symbol,
            name: function_name,
            parameters,
            return_type,
            body,
            visibility: Visibility::Public,
            effects: crate::tast::node::FunctionEffects::default(),
            type_parameters: Vec::new(), // TODO: Convert type parameters
            source_location: self.context.create_location(),
            metadata: FunctionMetadata::default(),
        })
    }

    /// Lower a parameter
    fn lower_parameter(&mut self, parameter: &FunctionParam) -> LoweringResult<TypedParameter> {
        let param_name = self.context.intern_string(&parameter.name);
        let param_symbol = self.context.symbol_table.create_variable(param_name);

        let param_type = if let Some(type_annotation) = &parameter.type_hint {
            self.lower_type(type_annotation)?
        } else {
            self.context.type_table.borrow().dynamic_type()
        };

        let default_value = if let Some(default) = &parameter.default_value {
            Some(self.lower_expression(default)?)
        } else {
            None
        };

        Ok(TypedParameter {
            symbol_id: param_symbol,
            name: param_name,
            param_type: param_type,
            is_optional: parameter.optional,
            default_value,
            mutability: crate::tast::Mutability::Immutable,
            source_location: self.context.create_location(),
        })
    }

    /// Lower a type annotation
    fn lower_type(&mut self, type_annotation: &Type) -> LoweringResult<TypeId> {
        match type_annotation {
            Type::Path { path, params, .. } => {
                let name = if path.package.is_empty() {
                    path.name.clone()
                } else {
                    format!("{}.{}", path.package.join("."), path.name)
                };
                
                // Check if it's a type parameter
                if let Some(type_param) = self.context.resolve_type_parameter(&name) {
                    return Ok(type_param);
                }

                // Try to resolve as a built-in type
                if let Some(builtin_type) = self.resolve_builtin_type(&name) {
                    return Ok(builtin_type);
                }

                let interned_name = self.context.intern_string(&name);
                // Try to resolve as a user-defined type
                if let Some(symbol) = self.context.symbol_table.lookup_symbol(
                    self.context.current_scope,
                    interned_name
                ) {
                    // Process type arguments if present
                    if !params.is_empty() {
                        let type_arg_ids = params.iter()
                            .map(|arg| self.lower_type(arg))
                            .collect::<Result<Vec<_>, _>>()?;
                        
                        return Ok(self.context.type_table.borrow_mut().create_generic_instance(TypeId::invalid(), type_arg_ids));
                    }
                    
                    // For now, return a default type
                    return Ok(self.context.type_table.borrow().dynamic_type());
                }

                Err(LoweringError::UnresolvedType {
                    type_name: name.clone(),
                    location: self.context.create_location(),
                })
            }
            Type::Function { params, ret, .. } => {
                let param_types = params.iter()
                    .map(|param| self.lower_type(param))
                    .collect::<Result<Vec<_>, _>>()?;
                
                let return_type_id = self.lower_type(ret)?;
                Ok(self.context.type_table.borrow_mut().create_function_type(param_types, return_type_id))
            }
            Type::Anonymous { fields, .. } => {
                // Handle anonymous object types
                let mut field_types = Vec::new();
                for field in fields {
                    let field_type_id = self.lower_type(&field.type_hint)?;
                    field_types.push((field.name.clone(), field_type_id));
                }
                // For now, create a placeholder type for anonymous objects
                Ok(self.context.type_table.borrow().dynamic_type())
            }
            Type::Optional { inner, .. } => {
                let inner_type_id = self.lower_type(inner)?;
                Ok(self.context.type_table.borrow_mut().create_optional_type(inner_type_id))
            }
            Type::Parenthesis { inner, .. } => {
                // Just unwrap parentheses
                self.lower_type(inner)
            }
            Type::Intersection { left, right, .. } => {
                let left_type_id = self.lower_type(left)?;
                let right_type_id = self.lower_type(right)?;
                Ok(self.context.type_table.borrow_mut().create_type(crate::tast::core::TypeKind::Intersection {
                    types: vec![left_type_id, right_type_id]
                }))
            }
            Type::Wildcard { .. } => {
                // Wildcard types are used in type parameters, return Dynamic for now
                Ok(self.context.type_table.borrow().dynamic_type())
            }
        }
    }

    /// Resolve built-in types
    fn resolve_builtin_type(&self, name: &str) -> Option<TypeId> {
        let type_table = self.context.type_table.borrow();
        match name {
            "Int" => Some(type_table.int_type()),
            "Float" => Some(type_table.float_type()),
            "String" => Some(type_table.string_type()),
            "Bool" => Some(type_table.bool_type()),
            "Dynamic" => Some(type_table.dynamic_type()),
            "Void" => Some(type_table.void_type()),
            _ => None,
        }
    }

    /// Lower modifiers - simplified
    fn lower_modifiers(&mut self, _modifiers: &[Modifier]) -> LoweringResult<Vec<()>> {
        Ok(Vec::new())
    }

    /// Lower an expression as a statement
    fn lower_expression_as_statement(&mut self, expr: &Expr) -> LoweringResult<TypedStatement> {
        let typed_expr = self.lower_expression(expr)?;
        Ok(TypedStatement::Expression {
            expression: typed_expr,
            source_location: self.context.create_location(),
        })
    }

    /// Lower a statement (placeholder - not used with new parser)
    fn lower_statement(&mut self, _statement: &str) -> LoweringResult<TypedStatement> {
        let location = self.context.create_location();
        
        // Placeholder implementation
        Ok(TypedStatement::Expression {
            expression: TypedExpression {
                expr_type: self.context.type_table.borrow().void_type(),
                kind: TypedExpressionKind::Null,
                usage: VariableUsage::Copy,
                lifetime_id: crate::tast::LifetimeId::first(),
                source_location: location,
                metadata: ExpressionMetadata::default(),
            },
            source_location: location,
        })
    }
    
    /// Placeholder for old statement lowering - not used with new parser
    fn _old_statement_lowering_placeholder(&mut self) {
        // This was the old statement lowering implementation
        // Not used with the new parser interface
    }

    /// Lower an expression
    fn lower_expression(&mut self, expression: &Expr) -> LoweringResult<TypedExpression> {
        let kind = match &expression.kind {
            ExprKind::Int(value) => {
                TypedExpressionKind::Literal {
                    value: LiteralValue::Int(*value)
                }
            }
            ExprKind::Float(value) => {
                TypedExpressionKind::Literal {
                    value: LiteralValue::Float(*value)
                }
            }
            ExprKind::String(value) => {
                TypedExpressionKind::Literal {
                    value: LiteralValue::String(value.clone())
                }
            }
            ExprKind::Bool(value) => {
                TypedExpressionKind::Literal {
                    value: LiteralValue::Bool(*value)
                }
            }
            ExprKind::Null => {
                TypedExpressionKind::Null
            }
            ExprKind::Regex { pattern, flags } => {
                TypedExpressionKind::Literal {
                    value: LiteralValue::RegexWithFlags {
                        pattern: pattern.clone(),
                        flags: flags.clone(),
                    }
                }
            }
            ExprKind::Ident(name) => {
                let id_name = self.context.intern_string(name);
                let symbol = self.context.symbol_table.lookup_symbol(
                    self.context.current_scope, 
                    id_name
                )
                    .ok_or_else(|| LoweringError::UnresolvedSymbol {
                        name: name.clone(),
                        location: self.context.create_location(),
                    })?;
                
                TypedExpressionKind::Variable {
                    symbol_id: symbol.id
                }
            }
            ExprKind::Binary { left, op, right } => {
                let left_expr = self.lower_expression(left)?;
                let right_expr = self.lower_expression(right)?;
                let typed_op = self.lower_binary_operator(op)?;

                TypedExpressionKind::BinaryOp {
                    left: Box::new(left_expr),
                    operator: typed_op,
                    right: Box::new(right_expr),
                }
            }
            ExprKind::Unary { op, expr } => {
                let operand_expr = self.lower_expression(expr)?;
                let typed_op = self.lower_unary_operator(op)?;

                TypedExpressionKind::UnaryOp {
                    operator: typed_op,
                    operand: Box::new(operand_expr),
                }
            }
            ExprKind::Call { expr, args } => {
                let func_expr = self.lower_expression(expr)?;
                let arg_exprs = args.iter()
                    .map(|arg| self.lower_expression(arg))
                    .collect::<Result<Vec<_>, _>>()?;

                TypedExpressionKind::FunctionCall {
                    function: Box::new(func_expr),
                    arguments: arg_exprs,
                    type_arguments: Vec::new(),
                }
            }
            ExprKind::Field { expr, field } => {
                let obj_expr = self.lower_expression(expr)?;
                let field_name = self.context.intern_string(field);
                let field_symbol = self.context.symbol_table.lookup_symbol(
                    self.context.current_scope,
                    field_name
                ).map(|s| s.id)
                    .ok_or_else(|| LoweringError::UnresolvedSymbol {
                        name: field.clone(),
                        location: self.context.create_location(),
                    })?;

                TypedExpressionKind::FieldAccess {
                    object: Box::new(obj_expr),
                    field_symbol,
                }
            }
            ExprKind::Index { expr, index } => {
                let array_expr = self.lower_expression(expr)?;
                let index_expr = self.lower_expression(index)?;

                TypedExpressionKind::ArrayAccess {
                    array: Box::new(array_expr),
                    index: Box::new(index_expr),
                }
            }
            ExprKind::Assign { left, op, right } => {
                let target_expr = self.lower_expression(left)?;
                let value_expr = self.lower_expression(right)?;
                
                match op {
                    parser::AssignOp::Assign => {
                        // Simple assignment: target = value
                        TypedExpressionKind::BinaryOp {
                            left: Box::new(target_expr),
                            operator: BinaryOperator::Assign,
                            right: Box::new(value_expr),
                        }
                    }
                    _ => {
                        // Compound assignment: target op= value
                        // This needs to be: target = target op value
                        let target_clone = target_expr.clone();
                        
                        // Map compound assignment operators to their corresponding binary operators
                        let binary_op = match op {
                            parser::AssignOp::AddAssign => BinaryOperator::Add,
                            parser::AssignOp::SubAssign => BinaryOperator::Sub,
                            parser::AssignOp::MulAssign => BinaryOperator::Mul,
                            parser::AssignOp::DivAssign => BinaryOperator::Div,
                            parser::AssignOp::ModAssign => BinaryOperator::Mod,
                            parser::AssignOp::AndAssign => BinaryOperator::BitAnd,
                            parser::AssignOp::OrAssign => BinaryOperator::BitOr,
                            parser::AssignOp::XorAssign => BinaryOperator::BitXor,
                            parser::AssignOp::ShlAssign => BinaryOperator::Shl,
                            parser::AssignOp::ShrAssign => BinaryOperator::Shr,
                            parser::AssignOp::UshrAssign => BinaryOperator::Shr, // UShr maps to Shr
                            parser::AssignOp::Assign => unreachable!(), // Handled above
                        };

                        // Create the binary operation: target op value
                        let binary_expr = TypedExpression {
                            expr_type: target_expr.expr_type,
                            kind: TypedExpressionKind::BinaryOp {
                                left: Box::new(target_clone),
                                operator: binary_op,
                                right: Box::new(value_expr),
                            },
                            usage: VariableUsage::Copy,
                            lifetime_id: crate::tast::LifetimeId::first(),
                            source_location: self.context.create_location(),
                            metadata: ExpressionMetadata::default(),
                        };

                        // Now assign the result back to target: target = (target op value)
                        TypedExpressionKind::BinaryOp {
                            left: Box::new(target_expr),
                            operator: BinaryOperator::Assign,
                            right: Box::new(binary_expr),
                        }
                    }
                }
            }
            ExprKind::New { type_path, params, args } => {
                // TODO: Handle type_path and params properly
                let class_type_id = TypeId::from_raw(1); // Placeholder
                let arg_exprs = args.iter()
                    .map(|arg| self.lower_expression(arg))
                    .collect::<Result<Vec<_>, _>>()?;

                TypedExpressionKind::New {
                    class_type: class_type_id,
                    arguments: arg_exprs,
                    type_arguments: Vec::new(),
                }
            }
            // Cast doesn't exist in ExprKind, remove this variant
            ExprKind::Ternary { cond, then_expr, else_expr } => {
                let cond_expr = self.lower_expression(cond)?;
                let then_expression = self.lower_expression(then_expr)?;
                let else_expression = Some(Box::new(self.lower_expression(else_expr)?));

                TypedExpressionKind::Conditional {
                    condition: Box::new(cond_expr),
                    then_expr: Box::new(then_expression),
                    else_expr: else_expression
                }
            }
            ExprKind::Block(block_elements) => {
                // Handle block expressions
                let mut statements = Vec::new();
                for elem in block_elements {
                    // Convert BlockElement to statement - need to handle this properly
                    // For now, use placeholder
                    statements.push(TypedStatement::Expression {
                        expression: TypedExpression {
                            expr_type: TypeId::from_raw(1),
                            kind: TypedExpressionKind::Literal { value: crate::tast::node::LiteralValue::Bool(false) },
                            usage: crate::tast::node::VariableUsage::Copy,
                            lifetime_id: crate::tast::LifetimeId::first(),
                            source_location: SourceLocation::unknown(),
                            metadata: crate::tast::node::ExpressionMetadata::default(),
                        },
                        source_location: SourceLocation::unknown(),
                    });
                }
                
                TypedExpressionKind::Block {
                    statements,
                    scope_id: self.context.enter_scope(ScopeKind::Block), // TODO: Create new scope for block
                }
            }
            ExprKind::If { cond, then_branch, else_branch } => {
                let cond_expr = self.lower_expression(cond)?;
                let then_expr = self.lower_expression(then_branch)?;
                let else_expr = if let Some(else_branch) = else_branch {
                    Some(Box::new(self.lower_expression(else_branch)?))
                } else {
                    None
                };

                TypedExpressionKind::Conditional  {
                    condition: Box::new(cond_expr),
                    then_expr: Box::new(then_expr),
                    else_expr
                }
            }
            ExprKind::While { cond, body } => {
                // Convert while expressions to statement form for proper CFG handling
                let cond_expr = self.lower_expression(cond)?;
                let body_stmt = self.convert_expression_to_statement(body)?;
                
                // Create a while statement and wrap it in a block expression
                let while_stmt = TypedStatement::While {
                    condition: cond_expr,
                    body: Box::new(body_stmt),
                    source_location: SourceLocation::unknown(),
                };
                
                // Return block expression containing the while statement
                TypedExpressionKind::Block {
                    statements: vec![while_stmt],
                    scope_id: ScopeId::from_raw(self.context.next_scope_id()),
                }
            }
            ExprKind::DoWhile { body, cond } => {
                // Convert do-while expressions to statement form
                let body_stmt = self.convert_expression_to_statement(body)?;
                let cond_expr = self.lower_expression(cond)?;
                
                // Create a do-while statement (add to TAST if missing)
                // For now, convert to while with initial execution
                let body_block = TypedStatement::Block {
                    statements: vec![body_stmt.clone()],
                    scope_id: ScopeId::from_raw(self.context.next_scope_id()),
                    source_location: SourceLocation::unknown(),
                };
                
                let while_stmt = TypedStatement::While {
                    condition: cond_expr,
                    body: Box::new(body_stmt),
                    source_location: SourceLocation::unknown(),
                };
                
                // Return block that executes body once, then while loop
                TypedExpressionKind::Block {
                    statements: vec![body_block, while_stmt],
                    scope_id: ScopeId::from_raw(self.context.next_scope_id()),
                }
            }
            ExprKind::For { var, key_var, iter, body } => {
                // Convert for-in expressions to statement form
                let iterable_expr = self.lower_expression(iter)?;
                let body_stmt = self.convert_expression_to_statement(body)?;
                
                let var_name = self.context.intern_string(var);
                let var_symbol = self.context.symbol_table.create_variable(var_name);
                
                // Create enhanced for statement (for-in style)
                // If TAST doesn't support for-in, convert to C-style for
                let for_stmt = self.convert_for_in_to_c_style_for(
                    var_symbol,
                    iterable_expr,
                    body_stmt,
                    SourceLocation::unknown(),
                )?;
                
                // Return block expression containing the for statement
                TypedExpressionKind::Block {
                    statements: vec![for_stmt],
                    scope_id: ScopeId::from_raw(self.context.next_scope_id()),
                }
            }
            ExprKind::Array(elements) => {
                let element_exprs = elements.iter()
                    .map(|elem| self.lower_expression(elem))
                    .collect::<Result<Vec<_>, _>>()?;

                TypedExpressionKind::ArrayLiteral {
                    elements: element_exprs,
                }
            }
            ExprKind::Return(expr) => {
                let return_expr = if let Some(expr) = expr {
                    Some(Box::new(self.lower_expression(expr)?))
                } else {
                    None
                };

                TypedExpressionKind::Return {
                    value: return_expr,
                }
            }
            ExprKind::Break => {
                TypedExpressionKind::Break
            }
            ExprKind::Continue => {
                TypedExpressionKind::Continue
            }
            // Is doesn't exist in ExprKind, remove this variant
            ExprKind::Throw(expr) => {
                let expression = self.lower_expression(expr)?;
                TypedExpressionKind::Throw {
                    expression: Box::new(expression),
                }
            }
            // For now, handle remaining expression types with placeholders
            _ => {
                // Return a placeholder expression for unhandled cases
                TypedExpressionKind::Literal {
                    value: LiteralValue::String("unhandled_expression".to_string()),
                }
            }
        };

        // Type will be determined during type checking
        let expr_type = self.context.type_table.borrow().dynamic_type();

        Ok(TypedExpression {
            expr_type,
            kind,
            usage: VariableUsage::Copy,
            lifetime_id: crate::tast::LifetimeId::first(),
            source_location: self.context.create_location(),
            metadata: ExpressionMetadata::default(),
        })
    }

    /// Lower a literal
    fn lower_literal(&mut self, literal: &parser::StringPart) -> LoweringResult<LiteralValue> {
        match literal {
            parser::StringPart::Literal(text) => Ok(LiteralValue::String(text.clone())),
            parser::StringPart::Interpolation(_) => {
                // For now, convert interpolated expressions to placeholders
                Ok(LiteralValue::String("${expr}".to_string()))
            }
        }
    }

    /// Lower a binary operator
    fn lower_binary_operator(&mut self, operator: &BinaryOp) -> LoweringResult<BinaryOperator> {
        match operator {
            BinaryOp::Add => Ok(BinaryOperator::Add),
            BinaryOp::Sub => Ok(BinaryOperator::Sub),
            BinaryOp::Mul => Ok(BinaryOperator::Mul),
            BinaryOp::Div => Ok(BinaryOperator::Div),
            BinaryOp::Mod => Ok(BinaryOperator::Mod),
            BinaryOp::Eq => Ok(BinaryOperator::Eq),
            BinaryOp::NotEq => Ok(BinaryOperator::Ne),
            BinaryOp::Lt => Ok(BinaryOperator::Lt),
            BinaryOp::Le => Ok(BinaryOperator::Le),
            BinaryOp::Gt => Ok(BinaryOperator::Gt),
            BinaryOp::Ge => Ok(BinaryOperator::Ge),
            BinaryOp::And => Ok(BinaryOperator::And),
            BinaryOp::Or => Ok(BinaryOperator::Or),
            BinaryOp::BitAnd => Ok(BinaryOperator::BitAnd),
            BinaryOp::BitOr => Ok(BinaryOperator::BitOr),
            BinaryOp::BitXor => Ok(BinaryOperator::BitXor),
            BinaryOp::Shl => Ok(BinaryOperator::Shl),
            BinaryOp::Shr => Ok(BinaryOperator::Shr),
            BinaryOp::Ushr => Ok(BinaryOperator::Shr), // Map to regular shift
            BinaryOp::Range => Ok(BinaryOperator::Add), // Map to add for now
            BinaryOp::Arrow => Ok(BinaryOperator::Add), // Map to add for now
            BinaryOp::Is => Ok(BinaryOperator::Eq), // Map to equality for now
            BinaryOp::NullCoal => Ok(BinaryOperator::Or), // Map to or for now
        }
    }

    /// Lower a unary operator
    fn lower_unary_operator(&mut self, operator: &UnaryOp) -> LoweringResult<UnaryOperator> {
        match operator {
            UnaryOp::Neg => Ok(UnaryOperator::Neg),
            UnaryOp::Not => Ok(UnaryOperator::Not),
            UnaryOp::BitNot => Ok(UnaryOperator::BitNot),
            UnaryOp::PreIncr => Ok(UnaryOperator::PreInc),
            UnaryOp::PostIncr => Ok(UnaryOperator::PostInc),
            UnaryOp::PreDecr => Ok(UnaryOperator::PreDec),
            UnaryOp::PostDecr => Ok(UnaryOperator::PostDec),
        }
    }

    // Property access handling removed for simplicity

    /// Get the errors collected during lowering
    pub fn get_errors(&self) -> &[LoweringError] {
        &self.context.errors
    }

    /// Convert an expression to a statement for proper CFG handling
    fn convert_expression_to_statement(&mut self, expr: &Expr) -> LoweringResult<TypedStatement> {
        let typed_expr = self.lower_expression(expr)?;
        
        // Wrap expression in an expression statement
        Ok(TypedStatement::Expression {
            expression: typed_expr,
            source_location: SourceLocation::unknown(),
        })
    }

    /// Convert for-in loop to C-style for loop for TAST compatibility
    fn convert_for_in_to_c_style_for(
        &mut self,
        variable: SymbolId,
        iterable: TypedExpression,
        body: TypedStatement,
        source_location: SourceLocation,
    ) -> LoweringResult<TypedStatement> {
        let iter_index_str = self.context.intern_string("__iter_index");
        // Create iterator variable: var i = 0
        let iterator_symbol = self.context.symbol_table.create_variable(
            iter_index_str
        );
        
        let zero_literal = TypedExpression {
            expr_type: self.context.type_table.borrow().int_type(),
            kind: TypedExpressionKind::Literal {
                value: LiteralValue::Int(0),
            },
            usage: VariableUsage::Copy,
            lifetime_id: LifetimeId::static_lifetime(),
            source_location,
            metadata: ExpressionMetadata::default(),
        };
        
        let init_stmt = TypedStatement::VarDeclaration {
            symbol_id: iterator_symbol,
            var_type: self.context.type_table.borrow().int_type(),
            initializer: Some(zero_literal),
            source_location,
            mutability: crate::tast::Mutability::Mutable
        };
        
        let length_str = self.context.intern_string("length");
        // Create condition: i < iterable.length
        let length_access = TypedExpression {
            expr_type: self.context.type_table.borrow().int_type(),
            kind: TypedExpressionKind::FieldAccess {
                object: Box::new(iterable.clone()),
                field_symbol: self.context.symbol_table.create_variable(
                    length_str
                ),
            },
            usage: VariableUsage::Copy,
            lifetime_id: LifetimeId::static_lifetime(),
            source_location,
            metadata: ExpressionMetadata::default(),
        };
        
        let iterator_var = TypedExpression {
            expr_type: self.context.type_table.borrow().int_type(),
            kind: TypedExpressionKind::Variable {
                symbol_id: iterator_symbol,
            },
            usage: VariableUsage::Copy,
            lifetime_id: LifetimeId::static_lifetime(),
            source_location,
            metadata: ExpressionMetadata::default(),
        };
        
        let condition = TypedExpression {
            expr_type: self.context.type_table.borrow().bool_type(),
            kind: TypedExpressionKind::BinaryOp {
                left: Box::new(iterator_var.clone()),
                operator: BinaryOperator::Lt,
                right: Box::new(length_access),
            },
            usage: VariableUsage::Copy,
            lifetime_id: LifetimeId::static_lifetime(),
            source_location,
            metadata: ExpressionMetadata::default(),
        };
        
        // Create update: i++
        let one_literal = TypedExpression {
            expr_type: self.context.type_table.borrow().int_type(),
            kind: TypedExpressionKind::Literal {
                value: LiteralValue::Int(1),
            },
            usage: VariableUsage::Copy,
            lifetime_id: LifetimeId::static_lifetime(),
            source_location,
            metadata: ExpressionMetadata::default(),
        };
        
        let update = TypedExpression {
            expr_type: self.context.type_table.borrow().int_type(),
            kind: TypedExpressionKind::BinaryOp {
                left: Box::new(iterator_var.clone()),
                operator: BinaryOperator::AddAssign,
                right: Box::new(one_literal),
            },
            usage: VariableUsage::Copy,
            lifetime_id: LifetimeId::static_lifetime(),
            source_location,
            metadata: ExpressionMetadata::default(),
        };
        
        // Create loop variable assignment: var variable = iterable[i]
        let array_access = TypedExpression {
            expr_type: iterable.expr_type, // Element type
            kind: TypedExpressionKind::ArrayAccess {
                array: Box::new(iterable),
                index: Box::new(iterator_var),
            },
            usage: VariableUsage::Copy,
            lifetime_id: LifetimeId::static_lifetime(),
            source_location,
            metadata: ExpressionMetadata::default(),
        };
        
        let loop_var_decl = TypedStatement::VarDeclaration {
            symbol_id: variable,
            var_type: array_access.expr_type,
            initializer: Some(array_access),
            source_location,
            mutability: crate::tast::Mutability::Mutable,
        };
        
        // Combine loop variable declaration with body
        let enhanced_body = TypedStatement::Block {
            statements: vec![loop_var_decl, body],
            scope_id: ScopeId::from_raw(self.context.next_scope_id()),
            source_location,
        };
        
        // Create C-style for loop
        Ok(TypedStatement::For {
            init: Some(Box::new(init_stmt)),
            condition: Some(condition),
            update: Some(update),
            body: Box::new(enhanced_body),
            source_location,
        })
    }
}

/// Convenience function to lower a Haxe file
pub fn lower_haxe_file(
    file: &HaxeFile,
    string_interner: &mut StringInterner,
    symbol_table: &mut SymbolTable,
    type_table: &RefCell<TypeTable>,
    scope_tree: &mut ScopeTree,
) -> LoweringResult<TypedFile> {
    let mut lowering = AstLowering::new(string_interner, symbol_table, type_table, scope_tree);
    lowering.lower_file(file)
}