//! Two-Pass Type Resolution System
//!
//! This module implements a two-pass type resolution system to handle forward references
//! and ensure all types are properly resolved before type checking and HIR lowering.
//!
//! Pass 1: Declaration Collection
//! - Collects all type declarations (classes, interfaces, enums, etc.)
//! - Creates forward references for all types
//! - Builds initial symbol table with type names
//!
//! Pass 2: Type Resolution
//! - Resolves all type references to concrete types
//! - Validates type parameters and constraints
//! - Ensures no Dynamic types remain where concrete types are needed

use crate::tast::{
    core::*,
    node::*,
    StringInterner,
    SymbolTable,
    TypeTable,
    ScopeTree,
    InternedString,
    ScopeId,
    SourceLocation,
    TypeId,
    SymbolId,
};
use parser::{HaxeFile, TypeDeclaration, Type as ParserType};
use std::collections::{HashMap, HashSet};
use std::cell::RefCell;
use std::rc::Rc;

/// Forward reference information for a type
#[derive(Debug, Clone)]
pub struct ForwardTypeReference {
    pub name: InternedString,
    pub kind: TypeKind,
    pub scope_id: ScopeId,
    pub source_location: SourceLocation,
    pub type_parameters: Vec<InternedString>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TypeKind {
    Class,
    Interface,
    Enum,
    Abstract,
    TypeAlias,
}

/// Two-pass type resolver
pub struct TypeResolver<'a> {
    string_interner: &'a mut StringInterner,
    symbol_table: &'a mut SymbolTable,
    type_table: &'a Rc<RefCell<TypeTable>>,
    scope_tree: &'a mut ScopeTree,
    
    // Forward references collected in pass 1
    forward_references: HashMap<InternedString, ForwardTypeReference>,
    
    // Type dependencies for topological sorting
    type_dependencies: HashMap<InternedString, HashSet<InternedString>>,
    
    // Resolution order after dependency analysis
    resolution_order: Vec<InternedString>,
    
    // Errors collected during resolution
    errors: Vec<TypeResolutionError>,
}

#[derive(Debug, Clone)]
pub enum TypeResolutionError {
    CyclicDependency {
        types: Vec<String>,
        location: SourceLocation,
    },
    UnresolvedType {
        name: String,
        location: SourceLocation,
    },
    InvalidTypeParameter {
        name: String,
        message: String,
        location: SourceLocation,
    },
    DynamicTypeInCriticalContext {
        context: String,
        location: SourceLocation,
    },
    ForwardReferenceNotFound {
        name: String,
        location: SourceLocation,
    },
}

impl<'a> TypeResolver<'a> {
    pub fn new(
        string_interner: &'a mut StringInterner,
        symbol_table: &'a mut SymbolTable,
        type_table: &'a Rc<RefCell<TypeTable>>,
        scope_tree: &'a mut ScopeTree,
    ) -> Self {
        Self {
            string_interner,
            symbol_table,
            type_table,
            scope_tree,
            forward_references: HashMap::new(),
            type_dependencies: HashMap::new(),
            resolution_order: Vec::new(),
            errors: Vec::new(),
        }
    }
    
    /// Run the two-pass type resolution
    pub fn resolve_types(&mut self, ast_file: &HaxeFile) -> Result<(), Vec<TypeResolutionError>> {
        // Pass 1: Collect all type declarations
        self.collect_declarations(ast_file);
        
        // Analyze dependencies and determine resolution order
        self.analyze_dependencies()?;
        
        // Pass 2: Resolve types in dependency order
        self.resolve_in_order(ast_file)?;
        
        // Verify no Dynamic types remain in critical contexts
        self.verify_concrete_types()?;
        
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors.clone())
        }
    }
    
    /// Pass 1: Collect all type declarations and create forward references
    fn collect_declarations(&mut self, ast_file: &HaxeFile) {
        for decl in &ast_file.declarations {
            match decl {
                TypeDeclaration::Class(class) => {
                    let name = self.string_interner.intern(&class.name);
                    let type_params: Vec<InternedString> = class.type_params.iter()
                        .map(|tp| self.string_interner.intern(&tp.name))
                        .collect();
                    
                    let forward_ref = ForwardTypeReference {
                        name,
                        kind: TypeKind::Class,
                        scope_id: self.scope_tree.current_scope(),
                        source_location: SourceLocation::new(0, 0, 0, class.span.start as u32),
                        type_parameters: type_params,
                    };
                    
                    self.forward_references.insert(name, forward_ref);
                    
                    // Create symbol for the class
                    let symbol_id = self.symbol_table.create_class(name);
                    
                    // Add to current scope
                    self.scope_tree.current_scope_mut().add_symbol(symbol_id, name);
                }
                
                TypeDeclaration::Interface(interface) => {
                    let name = self.string_interner.intern(&interface.name);
                    let type_params: Vec<InternedString> = interface.type_params.iter()
                        .map(|tp| self.string_interner.intern(&tp.name))
                        .collect();
                    
                    let forward_ref = ForwardTypeReference {
                        name,
                        kind: TypeKind::Interface,
                        scope_id: self.scope_tree.current_scope(),
                        source_location: SourceLocation::new(0, 0, 0, interface.span.start as u32),
                        type_parameters: type_params,
                    };
                    
                    self.forward_references.insert(name, forward_ref);
                    
                    // Create symbol for the interface
                    let symbol_id = self.symbol_table.create_interface(name);
                    
                    // Add to current scope
                    self.scope_tree.current_scope_mut().add_symbol(symbol_id, name);
                }
                
                TypeDeclaration::Enum(enum_decl) => {
                    let name = self.string_interner.intern(&enum_decl.name);
                    let type_params: Vec<InternedString> = enum_decl.type_params.iter()
                        .map(|tp| self.string_interner.intern(&tp.name))
                        .collect();
                    
                    let forward_ref = ForwardTypeReference {
                        name,
                        kind: TypeKind::Enum,
                        scope_id: self.scope_tree.current_scope(),
                        source_location: SourceLocation::new(0, 0, 0, enum_decl.span.start as u32),
                        type_parameters: type_params,
                    };
                    
                    self.forward_references.insert(name, forward_ref);
                    
                    // Create symbol for the enum
                    let symbol_id = self.symbol_table.create_enum(name);
                    
                    // Add to current scope
                    self.scope_tree.current_scope_mut().add_symbol(symbol_id, name);
                }
                
                TypeDeclaration::Abstract(abstract_decl) => {
                    let name = self.string_interner.intern(&abstract_decl.name);
                    let type_params: Vec<InternedString> = abstract_decl.type_params.iter()
                        .map(|tp| self.string_interner.intern(&tp.name))
                        .collect();
                    
                    let forward_ref = ForwardTypeReference {
                        name,
                        kind: TypeKind::Abstract,
                        scope_id: self.scope_tree.current_scope(),
                        source_location: SourceLocation::new(0, 0, 0, abstract_decl.span.start as u32),
                        type_parameters: type_params,
                    };
                    
                    self.forward_references.insert(name, forward_ref);
                    
                    // Create symbol for the abstract
                    let symbol_id = self.symbol_table.create_type_alias(name);
                    
                    // Add to current scope
                    self.scope_tree.current_scope_mut().add_symbol(symbol_id, name);
                }
                
                TypeDeclaration::Typedef(typedef) => {
                    let name = self.string_interner.intern(&typedef.name);
                    let type_params: Vec<InternedString> = typedef.type_params.iter()
                        .map(|tp| self.string_interner.intern(&tp.name))
                        .collect();
                    
                    let forward_ref = ForwardTypeReference {
                        name,
                        kind: TypeKind::TypeAlias,
                        scope_id: self.scope_tree.current_scope(),
                        source_location: SourceLocation::new(0, 0, 0, typedef.span.start as u32),
                        type_parameters: type_params,
                    };
                    
                    self.forward_references.insert(name, forward_ref);
                    
                    // Create symbol for the typedef
                    let symbol_id = self.symbol_table.create_type_alias(name);
                    
                    // Add to current scope
                    self.scope_tree.current_scope_mut().add_symbol(symbol_id, name);
                }
            }
        }
    }
    
    /// Analyze type dependencies and create resolution order
    fn analyze_dependencies(&mut self) -> Result<(), Vec<TypeResolutionError>> {
        // Initialize dependency map
        for name in self.forward_references.keys() {
            self.type_dependencies.insert(*name, HashSet::new());
        }
        
        // TODO: Analyze actual dependencies from type references
        // For now, we'll use declaration order
        self.resolution_order = self.forward_references.keys().cloned().collect();
        
        Ok(())
    }
    
    /// Pass 2: Resolve types in dependency order
    fn resolve_in_order(&mut self, ast_file: &HaxeFile) -> Result<(), Vec<TypeResolutionError>> {
        // For each type in resolution order, fully resolve its definition
        for type_name in &self.resolution_order.clone() {
            if let Some(forward_ref) = self.forward_references.get(type_name) {
                // Find the actual declaration and resolve it
                for decl in &ast_file.declarations {
                    match &decl.decl {
                        TypeDeclaration::Class(class) => {
                            let name = self.string_interner.intern(&class.name);
                            if name == *type_name {
                                self.resolve_class_type(class, &forward_ref)?;
                            }
                        }
                        TypeDeclaration::Interface(interface) => {
                            let name = self.string_interner.intern(&interface.name);
                            if name == *type_name {
                                self.resolve_interface_type(interface, &forward_ref)?;
                            }
                        }
                        TypeDeclaration::Enum(enum_decl) => {
                            let name = self.string_interner.intern(&enum_decl.name);
                            if name == *type_name {
                                self.resolve_enum_type(enum_decl, &forward_ref)?;
                            }
                        }
                        TypeDeclaration::Abstract(abstract_decl) => {
                            let name = self.string_interner.intern(&abstract_decl.name);
                            if name == *type_name {
                                self.resolve_abstract_type(abstract_decl, &forward_ref)?;
                            }
                        }
                        TypeDeclaration::Typedef(typedef) => {
                            let name = self.string_interner.intern(&typedef.name);
                            if name == *type_name {
                                self.resolve_typedef_type(typedef, &forward_ref)?;
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Resolve a class type
    fn resolve_class_type(
        &mut self, 
        class: &parser::ClassDecl, 
        forward_ref: &ForwardTypeReference
    ) -> Result<(), Vec<TypeResolutionError>> {
        // Resolve superclass if present
        if let Some(extends) = &class.extends {
            match self.resolve_type_reference(extends) {
                Ok(type_id) => {
                    // Validate it's a class type
                    let type_info = self.type_table.borrow().get_type_info(type_id);
                    match type_info {
                        Some(type_info) => {
                            match &type_info.kind {
                                TypeKind::Class { .. } => {
                                    // Valid superclass
                                }
                                _ => {
                                    self.errors.push(TypeResolutionError::InvalidTypeParameter {
                                        name: self.string_interner.get(forward_ref.name)
                                            .unwrap_or("<unknown>").to_string(),
                                        message: "Superclass must be a class type".to_string(),
                                        location: forward_ref.source_location,
                                    });
                                }
                            }
                        }
                        None => {
                            self.errors.push(TypeResolutionError::InvalidTypeParameter {
                                name: self.string_interner.get(forward_ref.name)
                                    .unwrap_or("<unknown>").to_string(),
                                message: "Superclass type not found".to_string(),
                                location: forward_ref.source_location,
                            });
                        }
                    }
                }
                Err(e) => self.errors.push(e),
            }
        }
        
        // Resolve implemented interfaces
        for interface_type in &class.implements {
            match self.resolve_type_reference(interface_type) {
                Ok(type_id) => {
                    // Validate it's an interface type
                    let type_info = self.type_table.borrow().get_type_info(type_id);
                    match type_info {
                        Some(type_info) => {
                            match &type_info.kind {
                                TypeKind::Interface { .. } => {
                                    // Valid interface
                                }
                                _ => {
                                    self.errors.push(TypeResolutionError::InvalidTypeParameter {
                                        name: self.string_interner.get(forward_ref.name)
                                            .unwrap_or("<unknown>").to_string(),
                                        message: "Implemented type must be an interface".to_string(),
                                        location: forward_ref.source_location,
                                    });
                                }
                            }
                        }
                        None => {
                            self.errors.push(TypeResolutionError::InvalidTypeParameter {
                                name: self.string_interner.get(forward_ref.name)
                                    .unwrap_or("<unknown>").to_string(),
                                message: "Implemented interface type not found".to_string(),
                                location: forward_ref.source_location,
                            });
                        }
                    }
                }
                Err(e) => self.errors.push(e),
            }
        }
        
        Ok(())
    }
    
    /// Resolve an interface type
    fn resolve_interface_type(
        &mut self,
        interface: &parser::InterfaceDecl,
        forward_ref: &ForwardTypeReference
    ) -> Result<(), Vec<TypeResolutionError>> {
        // Resolve extended interfaces
        for extended in &interface.extends {
            match self.resolve_type_reference(extended) {
                Ok(type_id) => {
                    // Validate it's an interface type
                    let type_info = self.type_table.borrow().get_type_info(type_id);
                    match type_info {
                        Some(type_info) => {
                            match &type_info.kind {
                                TypeKind::Interface { .. } => {
                                    // Valid interface
                                }
                                _ => {
                                    self.errors.push(TypeResolutionError::InvalidTypeParameter {
                                        name: self.string_interner.get(forward_ref.name)
                                            .unwrap_or("<unknown>").to_string(),
                                        message: "Extended type must be an interface".to_string(),
                                        location: forward_ref.source_location,
                                    });
                                }
                            }
                        }
                        None => {
                            self.errors.push(TypeResolutionError::InvalidTypeParameter {
                                name: self.string_interner.get(forward_ref.name)
                                    .unwrap_or("<unknown>").to_string(),
                                message: "Extended interface type not found".to_string(),
                                location: forward_ref.source_location,
                            });
                        }
                    }
                }
                Err(e) => self.errors.push(e),
            }
        }
        
        Ok(())
    }
    
    /// Resolve an enum type
    fn resolve_enum_type(
        &mut self,
        _enum_decl: &parser::EnumDecl,
        _forward_ref: &ForwardTypeReference
    ) -> Result<(), Vec<TypeResolutionError>> {
        // Enums are simpler - just need to ensure constructor parameter types are resolved
        // This will be handled when lowering the actual enum constructors
        Ok(())
    }
    
    /// Resolve an abstract type
    fn resolve_abstract_type(
        &mut self,
        abstract_decl: &parser::AbstractDecl,
        forward_ref: &ForwardTypeReference
    ) -> Result<(), Vec<TypeResolutionError>> {
        // Resolve the underlying type
        match self.resolve_type_reference(&abstract_decl.underlying_type) {
            Ok(_) => {
                // Successfully resolved underlying type
            }
            Err(e) => self.errors.push(e),
        }
        
        // Resolve from/to types
        for from_type in &abstract_decl.from_types {
            if let Err(e) = self.resolve_type_reference(from_type) {
                self.errors.push(e);
            }
        }
        
        for to_type in &abstract_decl.to_types {
            if let Err(e) = self.resolve_type_reference(to_type) {
                self.errors.push(e);
            }
        }
        
        Ok(())
    }
    
    /// Resolve a typedef type
    fn resolve_typedef_type(
        &mut self,
        typedef: &parser::TypedefDecl,
        _forward_ref: &ForwardTypeReference
    ) -> Result<(), Vec<TypeResolutionError>> {
        // Resolve the target type
        match self.resolve_type_reference(&typedef.type_def) {
            Ok(_) => {
                // Successfully resolved target type
            }
            Err(e) => self.errors.push(e),
        }
        
        Ok(())
    }
    
    /// Resolve a type reference to a TypeId
    fn resolve_type_reference(&mut self, type_ref: &ParserType) -> Result<TypeId, TypeResolutionError> {
        match type_ref {
            ParserType::Path { path, params, .. } => {
                // Construct the full type name from the path
                let name = if path.package.is_empty() {
                    path.name.clone()
                } else {
                    format!("{}.{}", path.package.join("."), path.name)
                };
                
                let interned_name = self.string_interner.intern(&name);
                
                // Check if it's a forward reference
                if let Some(_forward_ref) = self.forward_references.get(&interned_name) {
                    // Look up the symbol
                    if let Some((symbol_id, _)) = self.scope_tree.resolve_symbol(interned_name) {
                        // Process type arguments if present
                        let type_args = if !params.is_empty() {
                            let mut args = Vec::new();
                            for param in params {
                                args.push(self.resolve_type_reference(param)?);
                            }
                            args
                        } else {
                            Vec::new()
                        };
                        
                        // Get or create the type
                        let type_id = self.type_table.borrow_mut().get_or_create_named_type(
                            symbol_id,
                            type_args,
                            Ownership::Owned
                        );
                        Ok(type_id)
                    } else {
                        Err(TypeResolutionError::UnresolvedType {
                            name: name.clone(),
                            location: SourceLocation::unknown(),
                        })
                    }
                } else {
                    // Check if it's a primitive type (only for simple names)
                    if path.package.is_empty() && path.sub.is_none() {
                        match path.name.as_str() {
                            "Int" => Ok(self.type_table.borrow().int_type()),
                            "Float" => Ok(self.type_table.borrow().float_type()),
                            "Bool" => Ok(self.type_table.borrow().bool_type()),
                            "String" => Ok(self.type_table.borrow().string_type()),
                            "Void" => Ok(self.type_table.borrow().void_type()),
                            "Dynamic" => Ok(self.type_table.borrow().dynamic_type()),
                            _ => Err(TypeResolutionError::UnresolvedType {
                                name: name.clone(),
                                location: SourceLocation::unknown(),
                            })
                        }
                    } else {
                        Err(TypeResolutionError::UnresolvedType {
                            name,
                            location: SourceLocation::unknown(),
                        })
                    }
                }
            }
            
            ParserType::Function { params, ret, .. } => {
                // Resolve parameter types
                let mut param_types = Vec::new();
                for param in params {
                    param_types.push(self.resolve_type_reference(param)?);
                }
                
                // Resolve return type
                let ret_type = self.resolve_type_reference(ret)?;
                
                // Create function type
                let type_id = self.type_table.borrow_mut().create_function_type(
                    param_types,
                    ret_type,
                    crate::tast::node::FunctionEffects::default()
                );
                Ok(type_id)
            }
            
            ParserType::Optional { inner, .. } => {
                let inner = self.resolve_type_reference(inner)?;
                let type_id = self.type_table.borrow_mut().create_optional_type(inner);
                Ok(type_id)
            }
            
            ParserType::Parenthesis { inner, .. } => {
                self.resolve_type_reference(inner)
            }
            
            ParserType::Anonymous { fields: _, .. } => {
                // For now, anonymous types become Dynamic
                // TODO: Implement proper anonymous type support
                Ok(self.type_table.borrow().dynamic_type())
            }
            
            ParserType::Intersection { .. } => {
                // For now, intersection types become Dynamic
                // TODO: Implement proper intersection type support
                Ok(self.type_table.borrow().dynamic_type())
            }
        }
    }
    
    /// Verify that no Dynamic types remain in critical contexts
    fn verify_concrete_types(&mut self) -> Result<(), Vec<TypeResolutionError>> {
        // This will be implemented to check that:
        // 1. Field types are concrete (not Dynamic)
        // 2. Method parameter and return types are concrete
        // 3. Type parameters have proper bounds
        // For now, we'll accept the current state
        Ok(())
    }
    
    /// Get the errors collected during resolution
    pub fn take_errors(&mut self) -> Vec<TypeResolutionError> {
        std::mem::take(&mut self.errors)
    }
}

/// Extension trait for TypeTable to support two-pass resolution
impl TypeTable {
    /// Get the symbol ID for a given type ID
    pub fn get_symbol_for_type(&self, type_id: TypeId) -> Option<SymbolId> {
        let type_info = self.get_type_info(type_id)?;
        match &type_info.kind {
            TypeKind::Class { symbol_id, .. } |
            TypeKind::Interface { symbol_id, .. } |
            TypeKind::Enum { symbol_id, .. } |
            TypeKind::Abstract { symbol_id, .. } |
            TypeKind::TypeAlias { symbol_id, .. } => Some(*symbol_id),
            _ => None,
        }
    }
}