//! Monomorphization Pass for Generic Types
//!
//! This module implements lazy monomorphization - generating specialized versions
//! of generic functions and types on-demand when concrete type arguments are used.
//!
//! ## Strategy
//!
//! 1. **Lazy Instantiation**: Generate specializations only when actually used
//! 2. **Caching**: Use MonoKey (function + type_args) to cache generated instances
//! 3. **Type Substitution**: Replace TypeVar with concrete types throughout the function
//! 4. **Name Mangling**: Generate unique names like `Container_Int`, `Container_String`
//!
//! ## Integration Points
//!
//! - Called during MIR-to-MIR transformation before codegen
//! - Uses SymbolFlags::GENERIC to identify monomorphizable types
//! - Rewrites CallDirect instructions that target generic functions

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::functions::{IrFunctionSignature, IrParameter};
use super::modules::IrModule;
use super::{
    IrBasicBlock, IrBlockId, IrControlFlowGraph, IrFunction, IrFunctionId, IrInstruction,
    IrTerminator, IrType,
};

/// Key for caching monomorphized function instances
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MonoKey {
    /// The original generic function ID
    pub generic_func: IrFunctionId,
    /// Concrete type arguments used for instantiation
    pub type_args: Vec<IrType>,
}

impl MonoKey {
    pub fn new(generic_func: IrFunctionId, type_args: Vec<IrType>) -> Self {
        Self {
            generic_func,
            type_args,
        }
    }

    /// Generate a mangled name for this instantiation
    pub fn mangled_name(&self, base_name: &str) -> String {
        if self.type_args.is_empty() {
            return base_name.to_string();
        }

        let type_suffix: Vec<String> = self
            .type_args
            .iter()
            .map(|ty| Self::mangle_type(ty))
            .collect();

        format!("{}__{}", base_name, type_suffix.join("_"))
    }

    /// Mangle a type into a name-safe string
    fn mangle_type(ty: &IrType) -> String {
        match ty {
            IrType::Void => "void".to_string(),
            IrType::Bool => "bool".to_string(),
            IrType::I8 => "i8".to_string(),
            IrType::I16 => "i16".to_string(),
            IrType::I32 => "i32".to_string(),
            IrType::I64 => "i64".to_string(),
            IrType::U8 => "u8".to_string(),
            IrType::U16 => "u16".to_string(),
            IrType::U32 => "u32".to_string(),
            IrType::U64 => "u64".to_string(),
            IrType::F32 => "f32".to_string(),
            IrType::F64 => "f64".to_string(),
            IrType::String => "String".to_string(),
            IrType::Ptr(inner) => format!("Ptr{}", Self::mangle_type(inner)),
            IrType::Ref(inner) => format!("Ref{}", Self::mangle_type(inner)),
            IrType::Array(elem, size) => format!("Arr{}x{}", Self::mangle_type(elem), size),
            IrType::Slice(elem) => format!("Slice{}", Self::mangle_type(elem)),
            IrType::Struct { name, .. } => name.replace("::", "_"),
            IrType::Union { name, .. } => name.replace("::", "_"),
            IrType::Opaque { name, .. } => name.replace("::", "_"),
            IrType::Function { .. } => "Fn".to_string(),
            IrType::TypeVar(name) => name.clone(),
            IrType::Generic { base, type_args } => {
                let base_name = Self::mangle_type(base);
                let args: Vec<String> = type_args.iter().map(Self::mangle_type).collect();
                format!("{}__{}", base_name, args.join("_"))
            }
            IrType::Any => "Any".to_string(),
            IrType::Vector { element, count } => {
                format!("Vec{}x{}", Self::mangle_type(element), count)
            }
        }
    }
}

/// Statistics for monomorphization pass
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MonomorphizationStats {
    /// Number of generic functions found
    pub generic_functions_found: usize,
    /// Number of instantiations created
    pub instantiations_created: usize,
    /// Number of cache hits (reused existing instantiation)
    pub cache_hits: usize,
    /// Number of call sites rewritten
    pub call_sites_rewritten: usize,
    /// Types that were monomorphized
    pub monomorphized_types: Vec<String>,
}

/// The monomorphization engine
pub struct Monomorphizer {
    /// Cache of generated specializations: MonoKey -> specialized function ID
    instances: HashMap<MonoKey, IrFunctionId>,

    /// Mapping from type parameter names to concrete types (current substitution context)
    substitution_map: HashMap<String, IrType>,

    /// Next available function ID for new instantiations
    next_func_id: u32,

    /// Statistics
    stats: MonomorphizationStats,
}

impl Monomorphizer {
    pub fn new() -> Self {
        Self {
            instances: HashMap::new(),
            substitution_map: HashMap::new(),
            next_func_id: 10000, // Start high to avoid conflicts
            stats: MonomorphizationStats::default(),
        }
    }

    /// Get statistics about the monomorphization pass
    pub fn stats(&self) -> &MonomorphizationStats {
        &self.stats
    }

    /// Run monomorphization on an entire module
    ///
    /// This will:
    /// 1. Identify all generic functions (those with type_params)
    /// 2. Find all call sites that use generic functions with concrete type args
    /// 3. Generate specialized versions and rewrite call sites
    pub fn monomorphize_module(&mut self, module: &mut IrModule) {
        // Phase 1: Identify generic functions
        let generic_funcs: Vec<IrFunctionId> = module
            .functions
            .iter()
            .filter(|(_, func)| !func.signature.type_params.is_empty())
            .map(|(id, _)| *id)
            .collect();

        self.stats.generic_functions_found = generic_funcs.len();

        if generic_funcs.is_empty() {
            return; // No generic functions to monomorphize
        }

        // Phase 2: Collect all instantiation requests
        let instantiation_requests = self.collect_instantiation_requests(module, &generic_funcs);

        // Phase 3: Generate specialized functions
        let mut new_functions: Vec<IrFunction> = Vec::new();
        for (key, call_sites) in &instantiation_requests {
            if let Some(generic_func) = module.functions.get(&key.generic_func) {
                let specialized = self.instantiate(generic_func, &key.type_args);
                new_functions.push(specialized);
            }
        }

        // Phase 4: Add new functions to module
        for func in new_functions {
            module.functions.insert(func.id, func);
        }

        // Phase 5: Rewrite call sites
        self.rewrite_call_sites(module, &instantiation_requests);
    }

    /// Collect all places where generic functions are called with concrete types
    fn collect_instantiation_requests(
        &self,
        module: &IrModule,
        generic_funcs: &[IrFunctionId],
    ) -> HashMap<MonoKey, Vec<CallSiteLocation>> {
        let mut requests: HashMap<MonoKey, Vec<CallSiteLocation>> = HashMap::new();

        for (func_id, function) in &module.functions {
            for (block_id, block) in &function.cfg.blocks {
                for (inst_idx, inst) in block.instructions.iter().enumerate() {
                    if let Some((target_func, type_args)) =
                        self.extract_generic_call(inst, generic_funcs, function)
                    {
                        let key = MonoKey::new(target_func, type_args);
                        let location = CallSiteLocation {
                            function_id: *func_id,
                            block_id: *block_id,
                            instruction_index: inst_idx,
                        };
                        requests.entry(key).or_default().push(location);
                    }
                }
            }
        }

        requests
    }

    /// Extract generic call information from an instruction
    fn extract_generic_call(
        &self,
        inst: &IrInstruction,
        generic_funcs: &[IrFunctionId],
        _context_func: &IrFunction,
    ) -> Option<(IrFunctionId, Vec<IrType>)> {
        match inst {
            IrInstruction::CallDirect {
                func_id, type_args, ..
            } => {
                if generic_funcs.contains(func_id) && !type_args.is_empty() {
                    Some((*func_id, type_args.clone()))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Generate a specialized version of a generic function
    pub fn instantiate(&mut self, generic_func: &IrFunction, type_args: &[IrType]) -> IrFunction {
        let key = MonoKey::new(generic_func.id, type_args.to_vec());

        // Check cache
        if let Some(&existing_id) = self.instances.get(&key) {
            self.stats.cache_hits += 1;
            // Return a dummy - the real function is already in the module
            // This shouldn't happen in normal flow since we check before calling
            let mut dummy = generic_func.clone();
            dummy.id = existing_id;
            return dummy;
        }

        // Build substitution map: type_param_name -> concrete_type
        self.substitution_map.clear();
        for (param, arg) in generic_func.signature.type_params.iter().zip(type_args) {
            self.substitution_map
                .insert(param.name.clone(), arg.clone());
        }

        // Clone and specialize the function
        let new_id = IrFunctionId(self.next_func_id);
        self.next_func_id += 1;

        let mut specialized = generic_func.clone();
        specialized.id = new_id;
        specialized.name = key.mangled_name(&generic_func.name);

        // Clear type params - this is now a concrete function
        specialized.signature.type_params.clear();

        // Substitute types in signature
        specialized.signature = self.substitute_signature(&specialized.signature);

        // Substitute types in locals
        for (_, local) in specialized.locals.iter_mut() {
            local.ty = self.substitute_type(&local.ty);
        }

        // Substitute types in register_types
        let mut new_register_types = HashMap::new();
        for (id, ty) in &specialized.register_types {
            new_register_types.insert(*id, self.substitute_type(ty));
        }
        specialized.register_types = new_register_types;

        // Substitute types in CFG instructions
        self.substitute_cfg(&mut specialized.cfg);

        // Cache the result
        self.instances.insert(key.clone(), new_id);
        self.stats.instantiations_created += 1;
        self.stats
            .monomorphized_types
            .push(specialized.name.clone());

        specialized
    }

    /// Substitute types in a function signature
    fn substitute_signature(&self, sig: &IrFunctionSignature) -> IrFunctionSignature {
        IrFunctionSignature {
            parameters: sig
                .parameters
                .iter()
                .map(|p| IrParameter {
                    name: p.name.clone(),
                    ty: self.substitute_type(&p.ty),
                    reg: p.reg,
                    by_ref: p.by_ref,
                })
                .collect(),
            return_type: self.substitute_type(&sig.return_type),
            calling_convention: sig.calling_convention,
            can_throw: sig.can_throw,
            type_params: Vec::new(), // Cleared - now concrete
            uses_sret: sig.uses_sret,
        }
    }

    /// Recursively substitute type variables with concrete types
    fn substitute_type(&self, ty: &IrType) -> IrType {
        match ty {
            IrType::TypeVar(name) => self
                .substitution_map
                .get(name)
                .cloned()
                .unwrap_or_else(|| ty.clone()),
            IrType::Ptr(inner) => IrType::Ptr(Box::new(self.substitute_type(inner))),
            IrType::Ref(inner) => IrType::Ref(Box::new(self.substitute_type(inner))),
            IrType::Array(elem, size) => IrType::Array(Box::new(self.substitute_type(elem)), *size),
            IrType::Slice(elem) => IrType::Slice(Box::new(self.substitute_type(elem))),
            IrType::Function {
                params,
                return_type,
                varargs,
            } => IrType::Function {
                params: params.iter().map(|p| self.substitute_type(p)).collect(),
                return_type: Box::new(self.substitute_type(return_type)),
                varargs: *varargs,
            },
            IrType::Struct { name, fields } => IrType::Struct {
                name: name.clone(),
                fields: fields
                    .iter()
                    .map(|f| super::types::StructField {
                        name: f.name.clone(),
                        ty: self.substitute_type(&f.ty),
                        offset: f.offset,
                    })
                    .collect(),
            },
            IrType::Union { name, variants } => IrType::Union {
                name: name.clone(),
                variants: variants
                    .iter()
                    .map(|v| super::types::UnionVariant {
                        name: v.name.clone(),
                        tag: v.tag,
                        fields: v.fields.iter().map(|f| self.substitute_type(f)).collect(),
                    })
                    .collect(),
            },
            IrType::Generic { base, type_args } => {
                let new_base = self.substitute_type(base);
                let new_args: Vec<IrType> =
                    type_args.iter().map(|a| self.substitute_type(a)).collect();

                // If all type args are now concrete, we could potentially
                // resolve this to a concrete type, but for now keep as Generic
                IrType::Generic {
                    base: Box::new(new_base),
                    type_args: new_args,
                }
            }
            // Primitive types pass through unchanged
            _ => ty.clone(),
        }
    }

    /// Substitute types in all CFG instructions
    fn substitute_cfg(&self, cfg: &mut IrControlFlowGraph) {
        for (_, block) in cfg.blocks.iter_mut() {
            for inst in block.instructions.iter_mut() {
                self.substitute_instruction(inst);
            }
            self.substitute_terminator(&mut block.terminator);
        }
    }

    /// Substitute types in a single instruction
    fn substitute_instruction(&self, inst: &mut IrInstruction) {
        match inst {
            IrInstruction::Alloc { ty, .. } => {
                *ty = self.substitute_type(ty);
            }
            IrInstruction::Load { ty, .. } => {
                *ty = self.substitute_type(ty);
            }
            IrInstruction::Cast { from_ty, to_ty, .. } => {
                *from_ty = self.substitute_type(from_ty);
                *to_ty = self.substitute_type(to_ty);
            }
            IrInstruction::BitCast { ty, .. } => {
                *ty = self.substitute_type(ty);
            }
            IrInstruction::CallDirect { type_args, .. } => {
                // Substitute type args
                for arg in type_args.iter_mut() {
                    *arg = self.substitute_type(arg);
                }
            }
            IrInstruction::GetElementPtr { ty, .. } => {
                *ty = self.substitute_type(ty);
            }
            // Other instructions don't have type fields that need substitution
            _ => {}
        }
    }

    /// Substitute types in a terminator
    fn substitute_terminator(&self, _term: &mut IrTerminator) {
        // Most terminators don't have type information
        // Add cases here if needed
    }

    /// Rewrite call sites to use specialized functions
    fn rewrite_call_sites(
        &mut self,
        module: &mut IrModule,
        requests: &HashMap<MonoKey, Vec<CallSiteLocation>>,
    ) {
        for (key, locations) in requests {
            if let Some(&specialized_id) = self.instances.get(key) {
                for loc in locations {
                    if let Some(func) = module.functions.get_mut(&loc.function_id) {
                        if let Some(block) = func.cfg.blocks.get_mut(&loc.block_id) {
                            if let Some(inst) = block.instructions.get_mut(loc.instruction_index) {
                                if let IrInstruction::CallDirect {
                                    func_id, type_args, ..
                                } = inst
                                {
                                    *func_id = specialized_id;
                                    type_args.clear(); // No longer generic
                                    self.stats.call_sites_rewritten += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Default for Monomorphizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Location of a call site in the IR
#[derive(Debug, Clone)]
struct CallSiteLocation {
    function_id: IrFunctionId,
    block_id: IrBlockId,
    instruction_index: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mono_key_mangling() {
        let key = MonoKey::new(IrFunctionId(1), vec![IrType::I32, IrType::String]);
        assert_eq!(key.mangled_name("Container"), "Container__i32_String");
    }

    #[test]
    fn test_mono_key_nested_types() {
        let key = MonoKey::new(IrFunctionId(1), vec![IrType::Ptr(Box::new(IrType::I32))]);
        assert_eq!(key.mangled_name("Process"), "Process__Ptri32");
    }

    #[test]
    fn test_type_substitution() {
        let mut mono = Monomorphizer::new();
        mono.substitution_map.insert("T".to_string(), IrType::I32);

        let original = IrType::TypeVar("T".to_string());
        let substituted = mono.substitute_type(&original);
        assert_eq!(substituted, IrType::I32);
    }

    #[test]
    fn test_nested_type_substitution() {
        let mut mono = Monomorphizer::new();
        mono.substitution_map
            .insert("T".to_string(), IrType::String);

        let original = IrType::Ptr(Box::new(IrType::TypeVar("T".to_string())));
        let substituted = mono.substitute_type(&original);
        assert_eq!(substituted, IrType::Ptr(Box::new(IrType::String)));
    }
}
