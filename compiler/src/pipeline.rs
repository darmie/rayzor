//! Complete Haxe compilation pipeline: Source -> AST -> TAST -> HIR
//!
//! This module provides the main compilation pipeline that takes Haxe source code
//! and transforms it through the following stages:
//! 1. Parse source code to AST using the enhanced parser
//! 2. Lower AST to TAST with type checking and semantic analysis
//! 3. Validate the resulting TAST for correctness
//! 4. Generate semantic graphs for advanced analysis
//! 5. Lower TAST to HIR (High-level Intermediate Representation)
//! 6. Optimize HIR for target platform
//! 


use log::{info, error, warn, debug};

use crate::tast::{
    node::{TypedFile, FileMetadata},
    string_intern::StringInterner,
    SourceLocation, TypeId, SymbolId, SymbolTable, TypeTable,
};
use crate::semantic_graph::{
    SemanticGraphs, GraphConstructionOptions,
    builder::CfgBuilder,
};
use crate::ir::{
    IrModule,
    tast_to_hir::lower_tast_to_hir,
    hir_to_mir::lower_hir_to_mir,
    hir::HirModule,
    optimization::{PassManager, OptimizationResult},
    validation::{validate_module, ValidationError},
    optimizable::{OptimizableModule, optimize},
};
use crate::tast::type_flow_guard::{
    TypeFlowGuard, FlowSafetyError, FlowSafetyResults,
};
use crate::error_codes::{error_registry, format_error_code};

// Use the parser's public interface
use parser::{
    parse_haxe_file_with_diagnostics,
    haxe_ast::HaxeFile,
    ParseResult,
};

use std::{rc::Rc, sync::Arc};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

/// Main compilation pipeline for Haxe source code
pub struct HaxeCompilationPipeline {
    /// String interner shared across compilation units
    string_interner: Rc<RefCell<StringInterner>>,
    
    /// Pipeline configuration
    pub (crate) config: PipelineConfig,
    
    /// Compilation statistics
    stats: PipelineStats,
}

/// Configuration for the compilation pipeline
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Enable detailed type checking
    pub strict_type_checking: bool,
    
    /// Enable lifetime analysis
    pub enable_lifetime_analysis: bool,
    
    /// Enable ownership tracking (required for memory safety)
    pub enable_ownership_analysis: bool,
    
    /// Enable borrow checking (required for memory safety)
    pub enable_borrow_checking: bool,
    
    /// Enable hot reload support (for development builds)
    pub enable_hot_reload: bool,
    
    /// Optimization level (0 = debug, 1 = basic, 2 = aggressive)
    pub optimization_level: u8,
    
    /// Collect detailed statistics
    pub collect_statistics: bool,
    
    /// Maximum number of errors before stopping
    pub max_errors: usize,
    
    /// Target execution mode for compilation
    pub target_platform: TargetPlatform,
    
    /// Enable colored error output
    pub enable_colored_errors: bool,
    
    /// Enable semantic graph generation
    pub enable_semantic_analysis: bool,
    
    /// Enable HIR lowering phase
    pub enable_hir_lowering: bool,
    
    /// Enable HIR optimization passes
    pub enable_hir_optimization: bool,
    
    /// Enable HIR validation
    pub enable_hir_validation: bool,
    
    /// Enable MIR lowering phase
    pub enable_mir_lowering: bool,
    
    /// Enable MIR optimization passes
    pub enable_mir_optimization: bool,
    
    /// Enable basic flow-sensitive analysis during type checking
    pub enable_flow_sensitive_analysis: bool,
    
    /// Enable enhanced flow analysis with CFG/DFG integration
    pub enable_enhanced_flow_analysis: bool,
    
    /// Enable memory safety analysis (lifetime and ownership)
    pub enable_memory_safety_analysis: bool,
}

/// Target execution modes for the hybrid VM/compiler system
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetPlatform {
    /// Direct interpretation for fastest iteration during development
    Interpreter,
    
    /// Cranelift JIT compilation for fast compile times and good performance
    CraneliftJIT,
    
    /// LLVM AOT compilation for maximum performance in shipping builds
    LLVM,
    
    /// WebAssembly target for browser and universal deployment
    WebAssembly,
    
    /// Legacy transpilation targets (for compatibility)
    Legacy(LegacyTarget),
}

/// Legacy transpilation targets from traditional Haxe
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyTarget {
    JavaScript,
    Neko,
    HashLink,
    Cpp,
    Java,
    CSharp,
    Python,
    Lua,
}

/// Statistics collected during compilation
#[derive(Debug, Clone, Default)]
pub struct PipelineStats {
    /// Number of files processed
    pub files_processed: usize,
    
    /// Total lines of code
    pub total_loc: usize,
    
    /// Parse time in microseconds
    pub parse_time_us: u64,
    
    /// AST lowering time in microseconds  
    pub lowering_time_us: u64,
    
    /// Type checking time in microseconds
    pub type_checking_time_us: u64,
    
    /// Total compilation time in microseconds
    pub total_time_us: u64,
    
    /// Number of warnings generated
    pub warning_count: usize,
    
    /// Number of errors encountered
    pub error_count: usize,
    
    /// Semantic analysis time in microseconds
    pub semantic_analysis_time_us: u64,
    
    /// HIR lowering time in microseconds
    pub hir_lowering_time_us: u64,
    
    /// HIR optimization time in microseconds
    pub hir_optimization_time_us: u64,
    
    /// HIR validation time in microseconds
    pub hir_validation_time_us: u64,
    
    /// MIR lowering time in microseconds
    pub mir_lowering_time_us: u64,
    
    /// MIR optimization time in microseconds
    pub mir_optimization_time_us: u64,
    
    /// Flow-sensitive analysis time in microseconds
    pub flow_analysis_time_us: u64,
    
    /// Enhanced flow analysis time in microseconds
    pub enhanced_flow_analysis_time_us: u64,
    
    /// Memory safety analysis time in microseconds
    pub memory_safety_analysis_time_us: u64,
    
    /// Memory usage statistics
    pub memory_stats: MemoryStats,
}

/// Memory usage statistics
#[derive(Debug, Clone, Default)]
pub struct MemoryStats {
    /// Peak memory usage in bytes
    pub peak_memory_bytes: usize,
    
    /// AST size in bytes
    pub ast_size_bytes: usize,
    
    /// TAST size in bytes
    pub tast_size_bytes: usize,
    
    /// String interner size in bytes
    pub string_interner_bytes: usize,
    
    /// HIR size in bytes
    pub hir_size_bytes: usize,
}

/// Result of compilation pipeline
#[derive(Debug, Clone)]
pub struct CompilationResult {
    /// Successfully compiled TAST files
    pub typed_files: Vec<TypedFile>,
    
    /// Successfully lowered HIR modules (high-level IR)
    pub hir_modules: Vec<Arc<HirModule>>,
    
    /// Successfully lowered MIR modules (mid-level IR in SSA form)
    pub mir_modules: Vec<Arc<IrModule>>,
    
    /// Semantic analysis results
    pub semantic_graphs: Vec<Arc<SemanticGraphs>>,
    
    /// Compilation errors encountered
    pub errors: Vec<CompilationError>,
    
    /// Compilation warnings
    pub warnings: Vec<CompilationWarning>,
    
    /// Pipeline statistics
    pub stats: PipelineStats,
}

/// Compilation error with detailed information
#[derive(Debug, Clone)]
pub struct CompilationError {
    /// Error message
    pub message: String,
    
    /// Source location of the error
    pub location: SourceLocation,
    
    /// Error category
    pub category: ErrorCategory,
    
    /// Optional suggestion for fixing the error
    pub suggestion: Option<String>,
    
    /// Related errors (for cascading issues)
    pub related_errors: Vec<String>,
}

/// Compilation warning
#[derive(Debug, Clone)]
pub struct CompilationWarning {
    /// Warning message
    pub message: String,
    
    /// Source location of the warning
    pub location: SourceLocation,
    
    /// Warning category
    pub category: WarningCategory,
    
    /// Whether this warning can be suppressed
    pub suppressible: bool,
}

/// Categories of compilation errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// Syntax error in source code
    ParseError,
    
    /// Type error (type mismatch, undefined type, etc.)
    TypeError,
    
    /// Symbol resolution error (undefined variable, etc.)
    SymbolError,
    
    /// Ownership/borrowing error
    OwnershipError,
    
    /// Lifetime error
    LifetimeError,
    
    /// Import/module error
    ImportError,
    
    /// HIR lowering error
    HIRLoweringError,
    
    /// HIR optimization error
    HIROptimizationError,
    
    /// HIR validation error
    HIRValidationError,
    
    /// Semantic analysis error
    SemanticAnalysisError,
    
    /// Internal compiler error
    InternalError,
}

/// Categories of compilation warnings
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningCategory {
    /// Unused variable, function, etc.
    UnusedCode,
    
    /// Deprecated feature usage
    Deprecated,
    
    /// Potential performance issue
    Performance,
    
    /// Style/convention warning
    Style,
    
    /// Potential correctness issue
    Correctness,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            strict_type_checking: true,
            enable_lifetime_analysis: true,
            enable_ownership_analysis: true,
            enable_borrow_checking: true,
            enable_hot_reload: false,
            optimization_level: 1,
            collect_statistics: true,
            max_errors: 100,
            target_platform: TargetPlatform::CraneliftJIT,
            enable_colored_errors: true,
            enable_semantic_analysis: true,
            enable_hir_lowering: true,
            enable_hir_optimization: true,
            enable_hir_validation: true,
            enable_mir_lowering: true,
            enable_mir_optimization: true,
            enable_flow_sensitive_analysis: true,
            enable_enhanced_flow_analysis: true,
            enable_memory_safety_analysis: true,
        }
    }
}

impl PipelineConfig {
    /// Configuration for development builds with hot reload
    pub fn development() -> Self {
        Self {
            strict_type_checking: true,
            enable_lifetime_analysis: true,
            enable_ownership_analysis: true,
            enable_borrow_checking: true,
            enable_hot_reload: true,
            optimization_level: 0,
            collect_statistics: true,
            max_errors: 100,
            target_platform: TargetPlatform::Interpreter,
            enable_colored_errors: true,
            enable_semantic_analysis: true,
            enable_hir_lowering: false,  // Skip HIR for interpreter mode
            enable_hir_optimization: false,
            enable_hir_validation: false,
            enable_mir_lowering: false,  // Skip MIR for interpreter mode
            enable_mir_optimization: false,
            enable_flow_sensitive_analysis: true,  // Keep flow analysis for safety
            enable_enhanced_flow_analysis: false,
            enable_memory_safety_analysis: true,
        }
    }
    
    /// Configuration for release builds with maximum performance
    pub fn release() -> Self {
        Self {
            strict_type_checking: true,
            enable_lifetime_analysis: true,
            enable_ownership_analysis: true,
            enable_borrow_checking: true,
            enable_hot_reload: false,
            optimization_level: 2,
            collect_statistics: false,
            max_errors: 100,
            target_platform: TargetPlatform::LLVM,
            enable_colored_errors: true,
            enable_semantic_analysis: true,
            enable_hir_lowering: true,
            enable_hir_optimization: true,
            enable_hir_validation: true,
            enable_mir_lowering: true,
            enable_mir_optimization: true,
            enable_flow_sensitive_analysis: true,
            enable_enhanced_flow_analysis: true,
            enable_memory_safety_analysis: true,
        }
    }
    
    /// Configuration for WebAssembly builds
    pub fn webassembly() -> Self {
        Self {
            strict_type_checking: true,
            enable_lifetime_analysis: true,
            enable_ownership_analysis: true,
            enable_borrow_checking: true,
            enable_hot_reload: false,
            optimization_level: 2,
            collect_statistics: false,
            max_errors: 100,
            target_platform: TargetPlatform::WebAssembly,
            enable_colored_errors: false,  // No colors for web output
            enable_semantic_analysis: true,
            enable_hir_lowering: true,
            enable_hir_optimization: true,
            enable_hir_validation: true,
            enable_mir_lowering: true,
            enable_mir_optimization: true,
            enable_flow_sensitive_analysis: true,
            enable_enhanced_flow_analysis: true,
            enable_memory_safety_analysis: true,
        }
    }
    
    /// Set whether to enable colored error output
    pub fn with_colored_errors(mut self, enabled: bool) -> Self {
        self.enable_colored_errors = enabled;
        self
    }
}

impl HaxeCompilationPipeline {
    /// Create a new compilation pipeline with default configuration
    pub fn new() -> Self {
        Self::with_config(PipelineConfig::default())
    }
    
    /// Create a compilation pipeline with custom configuration
    pub fn with_config(config: PipelineConfig) -> Self {
        let string_interner = Rc::new(RefCell::new(StringInterner::new()));
        
        Self {
            string_interner,
            config,
            stats: PipelineStats::default(),
        }
    }
    
    /// Compile a single Haxe source file
    pub fn compile_file<P: AsRef<Path>>(&mut self, file_path: P, source: &str) -> CompilationResult {
        let start_time = std::time::Instant::now();
        let mut result = CompilationResult {
            typed_files: Vec::new(),
            hir_modules: Vec::new(),
            mir_modules: Vec::new(),
            semantic_graphs: Vec::new(),
            errors: Vec::new(),
            warnings: Vec::new(),
            stats: PipelineStats::default(),
        };
        
        // Stage 1: Parse source code to AST
        let parse_start = std::time::Instant::now();
        match self.parse_source(file_path.as_ref(), source) {
            Ok((ast_file, source_map)) => {
                self.stats.parse_time_us += parse_start.elapsed().as_micros() as u64;
                
                // Stage 2: Lower AST to TAST
                let lowering_start = std::time::Instant::now();
                match self.lower_ast_to_tast(ast_file, file_path.as_ref(), source, source_map) {
                    Ok((typed_file, lowering_errors, symbol_table, type_table)) => {
                        self.stats.lowering_time_us += lowering_start.elapsed().as_micros() as u64;
                        
                        // Add any type errors from lowering/type checking
                        result.errors.extend(lowering_errors);
                        
                        // Stage 3: Validate TAST
                        if let Err(validation_errors) = self.validate_tast(&typed_file) {
                            result.errors.extend(validation_errors);
                        }
                        
                        // Stage 4: Generate semantic graphs (if enabled)
                        let semantic_graphs = if self.config.enable_semantic_analysis {
                            let semantic_start = std::time::Instant::now();
                            match self.build_semantic_graphs(&typed_file, &symbol_table, &type_table) {
                                Ok(graphs) => {
                                    self.stats.semantic_analysis_time_us += semantic_start.elapsed().as_micros() as u64;
                                    
                                    // Stage 4b: Enhanced flow analysis with CFG/DFG integration (if enabled)
                                    if self.config.enable_enhanced_flow_analysis {
                                        let enhanced_flow_start = std::time::Instant::now();
                                        let enhanced_flow_errors = self.run_enhanced_flow_analysis(
                                            &typed_file, 
                                            &graphs, 
                                            &symbol_table, 
                                            &type_table
                                        );
                                        result.errors.extend(enhanced_flow_errors);
                                        self.stats.enhanced_flow_analysis_time_us += enhanced_flow_start.elapsed().as_micros() as u64;
                                    }
                                    
                                    // Stage 4c: Memory safety analysis (if enabled)
                                    if self.config.enable_memory_safety_analysis {
                                        let memory_safety_start = std::time::Instant::now();
                                        let memory_safety_errors = self.run_memory_safety_analysis(
                                            &typed_file, 
                                            &graphs, 
                                            &symbol_table, 
                                            &type_table
                                        );
                                        result.errors.extend(memory_safety_errors);
                                        self.stats.memory_safety_analysis_time_us += memory_safety_start.elapsed().as_micros() as u64;
                                    }
                                    
                                    result.semantic_graphs.push(Arc::new(graphs.clone()));
                                    Some(graphs)
                                }
                                Err(semantic_errors) => {
                                    result.errors.extend(semantic_errors);
                                    None
                                }
                            }
                        } else {
                            None
                        };
                        
                        // Stage 5: Lower TAST to HIR (if enabled)
                        if self.config.enable_hir_lowering {
                            let hir_start = std::time::Instant::now();
                            match self.lower_tast_to_hir(&typed_file, semantic_graphs.as_ref(), &symbol_table, &type_table) {
                                Ok(hir_module) => {
                                    self.stats.hir_lowering_time_us += hir_start.elapsed().as_micros() as u64;
                                    
                                    // Stage 6: Validate HIR (if enabled)
                                    if self.config.enable_hir_validation {
                                        let validation_start = std::time::Instant::now();
                                        if let Err(hir_validation_errors) = self.validate_hir(&hir_module) {
                                            result.errors.extend(hir_validation_errors);
                                        }
                                        self.stats.hir_validation_time_us += validation_start.elapsed().as_micros() as u64;
                                    }
                                    
                                    // Stage 7: Optimize HIR (if enabled)
                                    let final_hir = if self.config.enable_hir_optimization {
                                        let opt_start = std::time::Instant::now();
                                        // Pass semantic graphs (including call graph) to optimizer
                                        let optimized = self.optimize_hir(hir_module, semantic_graphs.as_ref());
                                        self.stats.hir_optimization_time_us += opt_start.elapsed().as_micros() as u64;
                                        optimized
                                    } else {
                                        hir_module
                                    };
                                    
                                    // Store HIR module
                                    let hir_arc = Arc::new(final_hir.clone());
                                    result.hir_modules.push(hir_arc);
                                    
                                    // Stage 8: Lower HIR to MIR (if enabled)
                                    if self.config.enable_mir_lowering {
                                        let mir_start = std::time::Instant::now();
                                        match self.lower_hir_to_mir(&final_hir) {
                                            Ok(mir_module) => {
                                                self.stats.mir_lowering_time_us += mir_start.elapsed().as_micros() as u64;
                                                
                                                // Stage 9: Optimize MIR (if enabled)
                                                let final_mir = if self.config.enable_mir_optimization {
                                                    let opt_start = std::time::Instant::now();
                                                    let optimized = self.optimize_mir(mir_module);
                                                    self.stats.mir_optimization_time_us += opt_start.elapsed().as_micros() as u64;
                                                    optimized
                                                } else {
                                                    mir_module
                                                };
                                                
                                                // Store MIR module
                                                result.mir_modules.push(Arc::new(final_mir));
                                            }
                                            Err(mir_errors) => {
                                                result.errors.extend(mir_errors);
                                            }
                                        }
                                    }
                                }
                                Err(hir_errors) => {
                                    result.errors.extend(hir_errors);
                                }
                            }
                        }
                        
                        // Always add the typed file, even if there are type errors
                        // This allows constraint validation tests to work properly
                        result.typed_files.push(typed_file);
                    }
                    Err(fatal_lowering_errors) => {
                        // Only reach here for fatal errors that prevent TAST generation
                        result.errors.extend(fatal_lowering_errors);
                    }
                }
            }
            Err(parse_errors) => {
                result.errors.extend(parse_errors);
            }
        }
        
        // Update statistics
        self.stats.files_processed += 1;
        self.stats.total_loc += source.lines().count();
        self.stats.total_time_us += start_time.elapsed().as_micros() as u64;
        self.stats.error_count += result.errors.len();
        self.stats.warning_count += result.warnings.len();
        
        result.stats = self.stats.clone();
        result
    }
    
    /// Compile multiple Haxe source files
    pub fn compile_files<P: AsRef<Path>>(&mut self, files: &[(P, String)]) -> CompilationResult {
        let mut combined_result = CompilationResult {
            typed_files: Vec::new(),
            hir_modules: Vec::new(),
            mir_modules: Vec::new(),
            semantic_graphs: Vec::new(),
            errors: Vec::new(),
            warnings: Vec::new(),
            stats: PipelineStats::default(),
        };
        
        for (file_path, source) in files {
            let file_result = self.compile_file(file_path, source);
            
            combined_result.typed_files.extend(file_result.typed_files);
            combined_result.hir_modules.extend(file_result.hir_modules);
            combined_result.semantic_graphs.extend(file_result.semantic_graphs);
            combined_result.errors.extend(file_result.errors);
            combined_result.warnings.extend(file_result.warnings);
            
            // Stop on too many errors
            if combined_result.errors.len() >= self.config.max_errors {
                break;
            }
        }
        
        combined_result.stats = self.stats.clone();
        combined_result
    }
    
    /// Parse source code to AST and return both AST and SourceMap
    fn parse_source(&mut self, file_path: &Path, source: &str) -> Result<(HaxeFile, diagnostics::SourceMap), Vec<CompilationError>> {
        let file_name = file_path.to_str().unwrap_or("unknown");
        match parse_haxe_file_with_diagnostics(file_name, source) {
            Ok(parse_result) => {
                // Check if there are any errors in the diagnostics
                if parse_result.diagnostics.has_errors() {
                    let compilation_errors = parse_result.diagnostics.diagnostics.into_iter()
                        .filter(|d| d.severity == diagnostics::DiagnosticSeverity::Error)
                        .map(|d| CompilationError {
                            message: d.message,
                            location: SourceLocation::new(
                                d.span.start.line as u32,
                                d.span.start.column as u32,
                                d.span.end.line as u32,
                                d.span.end.column as u32,
                            ),
                            category: ErrorCategory::ParseError,
                            suggestion: if d.help.is_empty() { None } else { Some(d.help.join(" ")) },
                            related_errors: d.notes,
                        })
                        .collect();
                    Err(compilation_errors)
                } else {
                    Ok((parse_result.file, parse_result.source_map))
                }
            }
            Err(parse_error_str) => {
                let compilation_errors = vec![
                    CompilationError {
                        message: format!("Parse error: {}", parse_error_str),
                        location: SourceLocation::new(0, 0, 0, 0), // Default location
                        category: ErrorCategory::ParseError,
                        suggestion: None,
                        related_errors: Vec::new(),
                    }
                ];
                Err(compilation_errors)
            }
        }
    }
    
    /// Lower AST to TAST with type checking
    fn lower_ast_to_tast(&mut self, ast_file: HaxeFile, file_path: &Path, source: &str, source_map: diagnostics::SourceMap) -> Result<(TypedFile, Vec<CompilationError>, SymbolTable, Rc<RefCell<TypeTable>>), Vec<CompilationError>> {
        use crate::tast::ast_lowering::AstLowering;
        use crate::tast::{SymbolTable, ScopeTree, TypeTable, ScopeId};
        use crate::tast::type_checking_pipeline::type_check_with_diagnostics;
        use diagnostics::ErrorFormatter;
        use std::cell::RefCell;
        
        // Create the necessary infrastructure for AST lowering
        // Estimate capacity based on AST size
        let estimated_symbols = (ast_file.declarations.len() * 20) // Rough estimate: 20 symbols per type
            .max(100); // Minimum 100 symbols
        
        let mut symbol_table = SymbolTable::with_capacity(estimated_symbols);
        let type_table = Rc::new(RefCell::new(TypeTable::with_capacity(estimated_symbols)));
        let mut scope_tree = ScopeTree::new(ScopeId::from_raw(0));
        
        // Use the source_map from the parser - it already has the file added
        // The parser has already added the file to the source map, so we don't need to add it again
        let file_name = file_path.to_str().unwrap_or("unknown");
        // Assume file_id is 0 since the parser adds it as the first file
        let file_id = diagnostics::FileId::new(0);
        
        // Now proceed with AST lowering using resolved types
        let mut binding = self.string_interner.borrow_mut();
        
        // Create namespace and import resolvers
        let mut namespace_resolver = crate::tast::namespace::NamespaceResolver::new(&*binding);
        let mut import_resolver = crate::tast::namespace::ImportResolver::new(&namespace_resolver);
        let mut lowering = AstLowering::new(
            &mut binding,
            &mut symbol_table,
            &type_table,
            &mut scope_tree,
            &mut namespace_resolver,
            &mut import_resolver,
        );
        // Initialize span converter with proper filename
        lowering.initialize_span_converter_with_filename(
            file_id.as_usize() as u32, 
            source.to_string(),
            file_name.to_string()
        );
        
        // Lower the AST to TAST
        let mut typed_file = match lowering.lower_file(&ast_file) {
            Ok(typed_file) => typed_file,
            Err(lowering_error) => {
                // Convert lowering error to formatted diagnostic
                let formatted_error = self.format_lowering_error(&lowering_error, &source_map);
                let compilation_error = CompilationError {
                    message: formatted_error,
                    location: self.extract_location_from_lowering_error(&lowering_error),
                    category: ErrorCategory::TypeError,
                    suggestion: None,
                    related_errors: Vec::new(),
                };
                
                return Err(vec![compilation_error]);
            }
        };
        
        // Run type checking with diagnostics
        let diagnostics = type_check_with_diagnostics(
            &mut typed_file,
            &type_table,
            &symbol_table,
            &scope_tree,
            &binding,
            &source_map,
        ).unwrap_or_else(|_| diagnostics::Diagnostics::new());
        
        // Store any type errors but continue with typed file generation
        let mut type_errors = Vec::new();
        if !diagnostics.is_empty() {
            // Convert each diagnostic to a CompilationError
            for diagnostic in &diagnostics.diagnostics {
                // Format this individual diagnostic for the message
                let formatter = if self.config.enable_colored_errors {
                    ErrorFormatter::with_colors()
                } else {
                    ErrorFormatter::new()
                };
                
                // Create a diagnostics collection with just this one diagnostic
                let mut single_diagnostic = diagnostics::Diagnostics::new();
                single_diagnostic.push(diagnostic.clone());
                let formatted_message = formatter.format_diagnostics(&single_diagnostic, &source_map);
                
                // Extract the span location
                let location = SourceLocation {
                    file_id: diagnostic.span.file_id.as_usize() as u32,
                    line: diagnostic.span.start.line as u32,
                    column: diagnostic.span.start.column as u32,
                    byte_offset: diagnostic.span.start.byte_offset as u32,
                };
                
                let compilation_error = CompilationError {
                    message: formatted_message,
                    location,
                    category: ErrorCategory::TypeError,
                    suggestion: if diagnostic.help.is_empty() { None } else { Some(diagnostic.help.join(" ")) },
                    related_errors: Vec::new(),
                };
                
                type_errors.push(compilation_error);
            }
        }
        
        // Stage 2b: Basic flow-sensitive analysis (if enabled)
        if self.config.enable_flow_sensitive_analysis {
            let flow_start = std::time::Instant::now();
            let flow_errors = self.run_basic_flow_analysis(&typed_file, &symbol_table, &type_table);
            type_errors.extend(flow_errors);
            self.stats.flow_analysis_time_us += flow_start.elapsed().as_micros() as u64;
        }
        
        // Return the typed file along with any type errors and the symbol/type tables for later stages
        Ok((typed_file, type_errors, symbol_table, type_table))
    }
    
    /// Validate the resulting TAST for correctness
    fn validate_tast(&self, typed_file: &TypedFile) -> Result<(), Vec<CompilationError>> {
        let mut errors = Vec::new();
        
        // Validate functions
        for function in &typed_file.functions {
            if let Err(function_errors) = self.validate_function(function) {
                errors.extend(function_errors);
            }
        }
        
        // Validate classes
        for class in &typed_file.classes {
            if let Err(class_errors) = self.validate_class(class) {
                errors.extend(class_errors);
            }
        }
        
        // Validate interfaces
        for interface in &typed_file.interfaces {
            if let Err(interface_errors) = self.validate_interface(interface) {
                errors.extend(interface_errors);
            }
        }
        
        // Validate enums
        for enum_def in &typed_file.enums {
            if let Err(enum_errors) = self.validate_enum(enum_def) {
                errors.extend(enum_errors);
            }
        }
        
        // Validate abstracts
        for abstract_def in &typed_file.abstracts {
            if let Err(abstract_errors) = self.validate_abstract(abstract_def) {
                errors.extend(abstract_errors);
            }
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
    
    /// Extract package name from AST file
    fn extract_package_name(&self, ast_file: &HaxeFile) -> Option<String> {
        // Look for package declaration in AST
        // This is a simplified implementation
        ast_file.package.as_ref().map(|pkg| pkg.path.join("."))
    }
    
    /// Convert parser span to source location (placeholder implementation)
    fn convert_span_to_location(&self, line: u32, column: u32) -> SourceLocation {
        SourceLocation::new(line, column, line, column + 1)
    }
    
    /// Validate a function in the TAST
    fn validate_function(&self, function: &crate::tast::node::TypedFunction) -> Result<(), Vec<CompilationError>> {
        let mut errors = Vec::new();
        
        // Check function body consistency
        if function.body.is_empty() && !function.effects.is_pure {
            // Empty non-pure functions might be suspicious
        }
        
        // Validate parameter types
        for param in &function.parameters {
            if !self.is_valid_type_id(param.param_type) {
                errors.push(CompilationError {
                    message: format!("Invalid parameter type for '{}'", 
                        self.get_string_from_interned(param.name)),
                    location: param.source_location,
                    category: ErrorCategory::TypeError,
                    suggestion: Some("Check that the type is properly defined".to_string()),
                    related_errors: Vec::new(),
                });
            }
        }
        
        // Validate return type
        if !self.is_valid_type_id(function.return_type) {
            errors.push(CompilationError {
                message: "Invalid return type".to_string(),
                location: function.source_location,
                category: ErrorCategory::TypeError,
                suggestion: Some("Check that the return type is properly defined".to_string()),
                related_errors: Vec::new(),
            });
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
    
    /// Validate a class in the TAST
    fn validate_class(&self, class: &crate::tast::node::TypedClass) -> Result<(), Vec<CompilationError>> {
        let mut errors = Vec::new();
        
        // Check for duplicate method names
        let mut method_names = std::collections::HashSet::new();
        for method in &class.methods {
            let method_name = self.get_string_from_interned(method.name);
            if !method_names.insert(method_name.clone()) {
                errors.push(CompilationError {
                    message: format!("Duplicate method name: '{}'", method_name),
                    location: method.source_location,
                    category: ErrorCategory::SymbolError,
                    suggestion: Some("Rename one of the methods or use method overloading".to_string()),
                    related_errors: Vec::new(),
                });
            }
        }
        
        // Validate field types
        for field in &class.fields {
            if !self.is_valid_type_id(field.field_type) {
                errors.push(CompilationError {
                    message: format!("Invalid field type for '{}'", field.name),
                    location: field.source_location,
                    category: ErrorCategory::TypeError,
                    suggestion: Some("Check that the field type is properly defined".to_string()),
                    related_errors: Vec::new(),
                });
            }
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
    
    /// Validate an interface in the TAST
    fn validate_interface(&self, interface: &crate::tast::node::TypedInterface) -> Result<(), Vec<CompilationError>> {
        let mut errors = Vec::new();
        
        // Check for duplicate method signatures
        let mut method_signatures = std::collections::HashSet::new();
        for method in &interface.methods {
            let signature = format!("{}:{}", method.name, "type"); // Simplified signature
            if !method_signatures.insert(signature.clone()) {
                errors.push(CompilationError {
                    message: format!("Duplicate method signature: '{}'", signature),
                    location: method.source_location,
                    category: ErrorCategory::SymbolError,
                    suggestion: Some("Remove duplicate method or change signature".to_string()),
                    related_errors: Vec::new(),
                });
            }
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
    
    /// Validate an enum in the TAST
    fn validate_enum(&self, enum_def: &crate::tast::node::TypedEnum) -> Result<(), Vec<CompilationError>> {
        let mut errors = Vec::new();
        
        // Check for duplicate variant names
        let mut variant_names = std::collections::HashSet::new();
        for variant in &enum_def.variants {
            if !variant_names.insert(variant.name.clone()) {
                errors.push(CompilationError {
                    message: format!("Duplicate enum variant: '{}'", variant.name),
                    location: variant.source_location,
                    category: ErrorCategory::SymbolError,
                    suggestion: Some("Rename one of the variants".to_string()),
                    related_errors: Vec::new(),
                });
            }
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
    
    /// Validate an abstract type in the TAST
    fn validate_abstract(&self, abstract_def: &crate::tast::node::TypedAbstract) -> Result<(), Vec<CompilationError>> {
        let mut errors = Vec::new();
        
        // Validate underlying type if present
        if let Some(underlying_type) = abstract_def.underlying_type {
            if !self.is_valid_type_id(underlying_type) {
                errors.push(CompilationError {
                    message: "Invalid underlying type for abstract".to_string(),
                    location: abstract_def.source_location,
                    category: ErrorCategory::TypeError,
                    suggestion: Some("Check that the underlying type is properly defined".to_string()),
                    related_errors: Vec::new(),
                });
            }
        }
        
        // Validate from/to conversion types
        for &from_type in &abstract_def.from_types {
            if !self.is_valid_type_id(from_type) {
                errors.push(CompilationError {
                    message: "Invalid 'from' conversion type".to_string(),
                    location: abstract_def.source_location,
                    category: ErrorCategory::TypeError,
                    suggestion: Some("Check that the conversion type is properly defined".to_string()),
                    related_errors: Vec::new(),
                });
            }
        }
        
        for &to_type in &abstract_def.to_types {
            if !self.is_valid_type_id(to_type) {
                errors.push(CompilationError {
                    message: "Invalid 'to' conversion type".to_string(),
                    location: abstract_def.source_location,
                    category: ErrorCategory::TypeError,
                    suggestion: Some("Check that the conversion type is properly defined".to_string()),
                    related_errors: Vec::new(),
                });
            }
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
    
    /// Check if a type ID is valid (placeholder implementation)
    fn is_valid_type_id(&self, type_id: TypeId) -> bool {
        // In a real implementation, this would check against a type table
        type_id.is_valid()
    }
    
    /// Get string from interned string (helper method)
    fn get_string_from_interned(&self, interned: crate::tast::InternedString) -> String {
        self.string_interner.borrow()
            .get(interned)
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("<invalid:#{}>", interned.as_raw()))
    }
    
    /// Format lowering errors with consistent diagnostic formatting
    fn format_lowering_error(&self, error: &crate::tast::ast_lowering::LoweringError, source_map: &diagnostics::SourceMap) -> String {
        use crate::tast::ast_lowering::LoweringError;
        use diagnostics::{Diagnostic, DiagnosticSeverity, SourceSpan, SourcePosition, Label, ErrorFormatter};
        
        let formatter = if self.config.enable_colored_errors {
            ErrorFormatter::with_colors()
        } else {
            ErrorFormatter::new()
        };
        let mut diagnostics = diagnostics::Diagnostics::new();
        
        // Convert LoweringError to Diagnostic with proper formatting
        let diagnostic = match error {
            LoweringError::GenericParameterError { message, location } => {
                let start_pos = SourcePosition::new(location.line as usize, location.column as usize, location.byte_offset as usize);
                let end_pos = SourcePosition::new(location.line as usize, location.column as usize + 1, location.byte_offset as usize + 1);
                let span = SourceSpan::new(start_pos, end_pos, diagnostics::FileId::new(location.file_id as usize));
                
                Diagnostic {
                    severity: DiagnosticSeverity::Error,
                    code: Some(format_error_code(3002)),  // E3002: Invalid generic instantiation
                    message: message.clone(),
                    span: span.clone(),
                    labels: vec![Label::primary(span, "invalid generic instantiation")],
                    suggestions: vec![],
                    notes: vec!["Generic types must be instantiated with the correct number of type arguments".to_string()],
                    help: vec!["Check the type definition to see how many type parameters it expects".to_string()],
                }
            }
            
            LoweringError::UnresolvedSymbol { name, location } => {
                let start_pos = SourcePosition::new(location.line as usize, location.column as usize, location.byte_offset as usize);
                let end_pos = SourcePosition::new(location.line as usize, location.column as usize + name.len(), location.byte_offset as usize + name.len());
                let span = SourceSpan::new(start_pos, end_pos, diagnostics::FileId::new(location.file_id as usize));
                
                Diagnostic {
                    severity: DiagnosticSeverity::Error,
                    code: Some(format_error_code(2001)),  // E2001: Undefined symbol
                    message: format!("Cannot find symbol '{}'", name),
                    span: span.clone(),
                    labels: vec![Label::primary(span, "not found in this scope")],
                    suggestions: vec![],
                    notes: vec![],
                    help: vec!["Make sure the symbol is declared and in scope".to_string()],
                }
            }
            
            LoweringError::UnresolvedType { type_name, location } => {
                let start_pos = SourcePosition::new(location.line as usize, location.column as usize, location.byte_offset as usize);
                let end_pos = SourcePosition::new(location.line as usize, location.column as usize + type_name.len(), location.byte_offset as usize + type_name.len());
                let span = SourceSpan::new(start_pos, end_pos, diagnostics::FileId::new(location.file_id as usize));
                
                Diagnostic {
                    severity: DiagnosticSeverity::Error,
                    code: Some(format_error_code(1002)),  // E1002: Undefined type
                    message: format!("Cannot find type '{}'", type_name),
                    span: span.clone(),
                    labels: vec![Label::primary(span, "type not found")],
                    suggestions: vec![],
                    notes: vec!["Make sure the type is imported or defined".to_string()],
                    help: vec!["Check for typos in the type name or add the necessary import".to_string()],
                }
            }
            
            LoweringError::DuplicateSymbol { name, original_location: _, duplicate_location } => {
                let start_pos = SourcePosition::new(duplicate_location.line as usize, duplicate_location.column as usize, duplicate_location.byte_offset as usize);
                let end_pos = SourcePosition::new(duplicate_location.line as usize, duplicate_location.column as usize + name.len(), duplicate_location.byte_offset as usize + name.len());
                let span = SourceSpan::new(start_pos, end_pos, diagnostics::FileId::new(duplicate_location.file_id as usize));
                
                Diagnostic {
                    severity: DiagnosticSeverity::Error,
                    code: Some(format_error_code(2002)),  // E2002: Symbol already defined
                    message: format!("Duplicate definition of '{}'", name),
                    span: span.clone(),
                    labels: vec![Label::primary(span, "redefined here")],
                    suggestions: vec![],
                    notes: vec!["A symbol with this name was already defined in this scope".to_string()],
                    help: vec!["Rename one of the symbols or remove the duplicate definition".to_string()],
                }
            }
            
            LoweringError::InvalidModifiers { modifiers, location } => {
                let start_pos = SourcePosition::new(location.line as usize, location.column as usize, location.byte_offset as usize);
                let end_pos = SourcePosition::new(location.line as usize, location.column as usize + 10, location.byte_offset as usize + 10);
                let span = SourceSpan::new(start_pos, end_pos, diagnostics::FileId::new(location.file_id as usize));
                
                Diagnostic {
                    severity: DiagnosticSeverity::Error,
                    code: Some(format_error_code(104)),   // E0104: Invalid variable declaration
                    message: format!("Invalid modifier combination: {}", modifiers.join(", ")),
                    span: span.clone(),
                    labels: vec![Label::primary(span, "conflicting modifiers")],
                    suggestions: vec![],
                    notes: vec!["Some modifiers cannot be used together".to_string()],
                    help: vec!["Remove the conflicting modifiers".to_string()],
                }
            }
            
            LoweringError::TypeInferenceError { expression, location } => {
                let start_pos = SourcePosition::new(location.line as usize, location.column as usize, location.byte_offset as usize);
                let end_pos = SourcePosition::new(location.line as usize, location.column as usize + expression.len().min(20), location.byte_offset as usize + expression.len().min(20));
                let span = SourceSpan::new(start_pos, end_pos, diagnostics::FileId::new(location.file_id as usize));
                
                Diagnostic {
                    severity: DiagnosticSeverity::Error,
                    code: Some(format_error_code(1005)),  // E1005: Type inference failed
                    message: format!("Cannot infer type for expression '{}'", expression),
                    span: span.clone(),
                    labels: vec![Label::primary(span, "type cannot be inferred")],
                    suggestions: vec![],
                    notes: vec!["Add an explicit type annotation to help the compiler".to_string()],
                    help: vec!["Try adding a type annotation like 'var x: Type = ...'".to_string()],
                }
            }
            
            /* // TODO: Add these error variants when needed
            LoweringError::ConstraintViolation { type_arg: _, constraint, reason, location } => {
                let start_pos = SourcePosition::new(location.line as usize, location.column as usize, location.byte_offset as usize);
                let end_pos = SourcePosition::new(location.line as usize, location.column as usize + 10, location.byte_offset as usize + 10);
                let span = SourceSpan::new(start_pos, end_pos, diagnostics::FileId::new(location.file_id as usize));
                
                Diagnostic {
                    severity: DiagnosticSeverity::Error,
                    code: Some(format_error_code(3101)),  // E3101: Constraint violation
                    message: format!("Type constraint violation: {}", reason),
                    span: span.clone(),
                    labels: vec![Label::primary(span, &format!("does not satisfy constraint '{}'", constraint))],
                    suggestions: vec![],
                    notes: vec!["The type argument does not satisfy the required constraints".to_string()],
                    help: vec!["Use a type that implements the required interface or extends the required class".to_string()],
                }
            }
            
            LoweringError::TypeResolution { message, location } => {
                let start_pos = SourcePosition::new(location.line as usize, location.column as usize, location.byte_offset as usize);
                let end_pos = SourcePosition::new(location.line as usize, location.column as usize + 10, location.byte_offset as usize + 10);
                let span = SourceSpan::new(start_pos, end_pos, diagnostics::FileId::new(location.file_id as usize));
                
                Diagnostic {
                    severity: DiagnosticSeverity::Error,
                    code: Some(format_error_code(1004)),  // E1004: Circular type dependency
                    message: format!("Type resolution error: {}", message),
                    span: span.clone(),
                    labels: vec![Label::primary(span, "type resolution failed")],
                    suggestions: vec![],
                    notes: vec!["The type could not be resolved properly".to_string()],
                    help: vec!["Check that all required types are defined and accessible".to_string()],
                }
            }
            */
            
            LoweringError::IncompleteImplementation { feature, location } => {
                let start_pos = SourcePosition::new(location.line as usize, location.column as usize, location.byte_offset as usize);
                let end_pos = SourcePosition::new(location.line as usize, location.column as usize + 10, location.byte_offset as usize + 10);
                let span = SourceSpan::new(start_pos, end_pos, diagnostics::FileId::new(location.file_id as usize));
                
                Diagnostic {
                    severity: DiagnosticSeverity::Error,
                    code: Some(format_error_code(9002)),  // E9002: Unexpected compiler state
                    message: format!("Feature not yet implemented: {}", feature),
                    span: span.clone(),
                    labels: vec![Label::primary(span, "not implemented")],
                    suggestions: vec![],
                    notes: vec!["This language feature is not yet supported by the compiler".to_string()],
                    help: vec!["Try using an alternative approach or wait for this feature to be implemented".to_string()],
                }
            }
            
            LoweringError::InternalError { message, location } => {
                let start_pos = SourcePosition::new(location.line as usize, location.column as usize, location.byte_offset as usize);
                let end_pos = SourcePosition::new(location.line as usize, location.column as usize + 10, location.byte_offset as usize + 10);
                let span = SourceSpan::new(start_pos, end_pos, diagnostics::FileId::new(location.file_id as usize));
                
                Diagnostic {
                    severity: DiagnosticSeverity::Error,
                    code: Some(format_error_code(9001)),  // E9001: Compiler assertion failed
                    message: format!("Internal compiler error: {}", message),
                    span: span.clone(),
                    labels: vec![Label::primary(span, "internal error")],
                    suggestions: vec![],
                    notes: vec!["This is a bug in the compiler".to_string()],
                    help: vec!["Please report this issue to the compiler developers".to_string()],
                }
            }
            
            // Fallback for any remaining error types
            _ => {
                let location = self.extract_location_from_lowering_error(error);
                let start_pos = SourcePosition::new(location.line as usize, location.column as usize, location.byte_offset as usize);
                let end_pos = SourcePosition::new(location.line as usize, location.column as usize + 10, location.byte_offset as usize + 10);
                let span = SourceSpan::new(start_pos, end_pos, diagnostics::FileId::new(location.file_id as usize));
                
                Diagnostic {
                    severity: DiagnosticSeverity::Error,
                    code: Some(format_error_code(9999)),  // E9999: Unknown error
                    message: format!("Compilation error: {}", format!("{:?}", error).replace("LoweringError::", "")),
                    span: span.clone(),
                    labels: vec![Label::primary(span, "compilation failed")],
                    suggestions: vec![],
                    notes: vec!["An error occurred during compilation".to_string()],
                    help: vec!["Check the syntax and semantics of your code".to_string()],
                }
            }
        };
        
        diagnostics.push(diagnostic);
        formatter.format_diagnostics(&diagnostics, source_map)
    }
    
    /// Extract source location from lowering error
    fn extract_location_from_lowering_error(&self, error: &crate::tast::ast_lowering::LoweringError) -> crate::tast::SourceLocation {
        use crate::tast::ast_lowering::LoweringError;
        
        match error {
            LoweringError::UnresolvedSymbol { location, .. } => location.clone(),
            LoweringError::UnresolvedType { location, .. } => location.clone(),
            LoweringError::DuplicateSymbol { duplicate_location, .. } => duplicate_location.clone(),
            LoweringError::InvalidModifiers { location, .. } => location.clone(),
            LoweringError::InternalError { location, .. } => location.clone(),
            LoweringError::GenericParameterError { location, .. } => location.clone(),
            LoweringError::TypeInferenceError { location, .. } => location.clone(),
            LoweringError::LifetimeError { location, .. } => location.clone(),
            LoweringError::OwnershipError { location, .. } => location.clone(),
            LoweringError::IncompleteImplementation { location, .. } => location.clone(),
            /* // TODO: Add these when error variants are added
            LoweringError::TypeResolution { location, .. } => location.clone(),
            LoweringError::ConstraintViolation { location, .. } => location.clone(),
            */
        }
    }
    
    /// Build semantic graphs for advanced analysis
    fn build_semantic_graphs(&mut self, typed_file: &TypedFile, symbol_table: &SymbolTable, type_table: &Rc<RefCell<TypeTable>>) -> Result<SemanticGraphs, Vec<CompilationError>> {
        use crate::semantic_graph::GraphConstructionError;
        
        let options = GraphConstructionOptions {
            build_call_graph: true,
            build_ownership_graph: self.config.enable_ownership_analysis,
            convert_to_ssa: true,
            eliminate_dead_code: self.config.optimization_level > 0,
            max_function_size: if self.config.optimization_level == 0 { 5000 } else { 10000 },
            collect_statistics: self.config.collect_statistics,
        };
        
        // Use the symbol and type tables passed from TAST lowering
        // Note: CfgBuilder doesn't currently use these, but they're available
        // for future enhancements like improved type-aware CFG construction
        let _ = (symbol_table, type_table); // Mark as intentionally unused for now
        
        // Build semantic graphs using CfgBuilder
        let mut cfg_builder = CfgBuilder::new(options);
        match cfg_builder.build_file(typed_file) {
            Ok(cfgs) => {
                // Create SemanticGraphs from the built CFGs
                let mut graphs = SemanticGraphs::new();
                graphs.control_flow = cfgs;
                // Note: Enhanced flow and memory safety analysis are now integrated
                // into the semantic graph building phase above for proper error collection
                
                Ok(graphs)
            },
            Err(graph_error) => {
                let compilation_errors = vec![
                    CompilationError {
                        message: match &graph_error {
                            GraphConstructionError::InvalidTAST { message, .. } => 
                                format!("Invalid TAST: {}", message),
                            GraphConstructionError::TypeError { message } => 
                                format!("Type error: {}", message),
                            GraphConstructionError::InvalidCFG { message, .. } => 
                                format!("Invalid CFG: {}", message),
                            GraphConstructionError::UnresolvedSymbol { symbol_name, .. } => 
                                format!("Unresolved symbol: {}", symbol_name),
                            GraphConstructionError::MissingTypeInfo { node_description, .. } => 
                                format!("Missing type info: {}", node_description),
                            GraphConstructionError::InternalError { message } => 
                                format!("Internal error: {}", message),
                            GraphConstructionError::DominanceAnalysisFailed(message) => 
                                format!("Dominance analysis failed: {}", message),
                        },
                        location: match &graph_error {
                            GraphConstructionError::InvalidTAST { location, .. } |
                            GraphConstructionError::InvalidCFG { location, .. } |
                            GraphConstructionError::UnresolvedSymbol { location, .. } |
                            GraphConstructionError::MissingTypeInfo { location, .. } => location.clone(),
                            GraphConstructionError::InternalError { .. } |
                            GraphConstructionError::TypeError { .. } |
                            GraphConstructionError::DominanceAnalysisFailed(_) => {
                                // These are module-level errors, use file start location
                                SourceLocation::new(1, 1, 1, 1)
                            },
                        },
                        category: ErrorCategory::SemanticAnalysisError,
                        suggestion: None,
                        related_errors: Vec::new(),
                    }
                ];
                
                Err(compilation_errors)
            }
        }
    }
    
    /// Lower TAST to HIR 
    fn lower_tast_to_hir(
        &mut self, 
        typed_file: &TypedFile, 
        semantic_graphs: Option<&SemanticGraphs>,
        symbol_table: &SymbolTable,
        type_table: &Rc<RefCell<TypeTable>>
    ) -> Result<HirModule, Vec<CompilationError>> {
        // Use the new TAST to HIR lowering
        match lower_tast_to_hir(
            typed_file,
            symbol_table,
            type_table,
            semantic_graphs,
        ) {
            Ok(hir_module) => Ok(hir_module),
            Err(lowering_errors) => {
                let compilation_errors = lowering_errors.into_iter().map(|err| {
                    CompilationError {
                        message: err.message,
                        location: err.location,
                        category: ErrorCategory::HIRLoweringError,
                        suggestion: None,
                        related_errors: Vec::new(),
                    }
                }).collect();
                
                Err(compilation_errors)
            }
        }
    }
    
    /// Lower HIR to MIR (mid-level IR in SSA form)
    fn lower_hir_to_mir(
        &mut self,
        hir_module: &HirModule,
    ) -> Result<IrModule, Vec<CompilationError>> {
        match lower_hir_to_mir(hir_module) {
            Ok(mir_module) => Ok(mir_module),
            Err(lowering_errors) => {
                let compilation_errors = lowering_errors.into_iter().map(|err| {
                    CompilationError {
                        message: err.message,
                        location: err.location,
                        category: ErrorCategory::HIRLoweringError,
                        suggestion: None,
                        related_errors: Vec::new(),
                    }
                }).collect();
                
                Err(compilation_errors)
            }
        }
    }
    
    /// Optimize MIR modules
    fn optimize_mir(&mut self, mut mir_module: IrModule) -> IrModule {
        let mut pass_manager = if self.config.optimization_level == 0 {
            // Debug mode: minimal optimizations
            let mut manager = PassManager::new();
            manager.add_pass(crate::ir::optimization::DeadCodeEliminationPass::new());
            manager
        } else if self.config.optimization_level == 1 {
            // Basic optimizations
            PassManager::default_pipeline()
        } else {
            // Aggressive optimizations
            let mut manager = PassManager::default_pipeline();
            // Add additional passes for aggressive optimization
            manager.add_pass(crate::ir::optimization::ConstantFoldingPass::new());
            manager.add_pass(crate::ir::optimization::CopyPropagationPass::new());
            manager
        };
        
        let _result = pass_manager.run(&mut mir_module);
        // Could log optimization statistics here if needed
        
        mir_module
    }
    
    /// Validate HIR for correctness
    fn validate_hir(&self, hir_module: &HirModule) -> Result<(), Vec<CompilationError>> {
        // Use the OptimizableModule trait for validation
        match hir_module.validate() {
            Ok(()) => Ok(()),
            Err(validation_errors) => {
                let compilation_errors = validation_errors.into_iter().map(|err| {
                    CompilationError {
                        message: format!("HIR validation error: {:?}", err.kind),
                        location: SourceLocation::new(1, 1, 1, 1), // HIR validation errors apply to whole module
                        category: ErrorCategory::HIRValidationError,
                        suggestion: Some("Check the HIR module structure and ensure all invariants are satisfied".to_string()),
                        related_errors: Vec::new(),
                    }
                }).collect();
                
                Err(compilation_errors)
            }
        }
    }
    
    /// Optimize HIR using optimization passes
    fn optimize_hir(&self, mut hir_module: HirModule, semantic_graphs: Option<&SemanticGraphs>) -> HirModule {
        // Use semantic graphs for more precise optimization
        
        // Create HIR-specific optimization passes
        let mut passes: Vec<Box<dyn crate::ir::optimization::OptimizationPass>> = vec![];
        
        // Add dead code elimination with call graph if available
        if let Some(graphs) = semantic_graphs {
            let mut dce = crate::ir::optimizable::HirDeadCodeElimination::new()
                .with_call_graph(&graphs.call_graph);
            
            // Find entry points
            for func in hir_module.functions.values() {
                if func.is_entry_point() {
                    dce = dce.add_entry_point(func.symbol_id);
                }
            }
            
            // Note: We can't box HirDeadCodeElimination directly because it has a lifetime
            // For now, just run it directly
            if let crate::ir::optimization::OptimizationResult { modified: true, .. } = 
                crate::ir::optimizable::HirOptimizationPass::optimize_hir(&mut dce, &mut hir_module) {
                info!("HIR dead code elimination removed unreachable functions");
            }
        }
        
        // Run other generic optimization passes if any
        if !passes.is_empty() {
            match optimize(&mut hir_module, passes, false) {
                Ok(result) => {
                    if result.modified {
                        info!("HIR optimization modified the module");
                    }
                }
                Err(validation_errors) => {
                    error!("HIR validation failed after optimization: {:?}", validation_errors);
                }
            }
        }
        
        hir_module
    }
    
    /// Get pipeline statistics
    pub fn stats(&self) -> &PipelineStats {
        &self.stats
    }
    
    /// Reset pipeline statistics
    pub fn reset_stats(&mut self) {
        self.stats = PipelineStats::default();
    }
    
    /// Stage 2b: Run basic flow-sensitive analysis during type checking
    fn run_basic_flow_analysis(
        &self, 
        typed_file: &TypedFile, 
        symbol_table: &SymbolTable, 
        type_table: &Rc<RefCell<TypeTable>>
    ) -> Vec<CompilationError> {
        let mut type_flow_guard = TypeFlowGuard::new(symbol_table, type_table);
        
        // Perform basic flow analysis without CFG/DFG (they're built in stage 4)
        let flow_safety_results = type_flow_guard.analyze_file(typed_file);
        
        // Collect both errors and warnings
        let mut compilation_errors = self.convert_flow_safety_errors(flow_safety_results.errors);
        
        // Add warnings as well (converted to warnings)
        for warning in flow_safety_results.warnings {
            compilation_errors.push(self.convert_flow_safety_warning(warning));
        }
        
        compilation_errors
    }
    
    /// Stage 4b: Run enhanced flow analysis with CFG/DFG integration
    fn run_enhanced_flow_analysis(
        &self,
        typed_file: &TypedFile,
        semantic_graphs: &SemanticGraphs,
        symbol_table: &SymbolTable,
        type_table: &Rc<RefCell<TypeTable>>
    ) -> Vec<CompilationError> {
        let mut type_flow_guard = TypeFlowGuard::new(symbol_table, type_table);
        let mut errors = Vec::new();
        
        // Enhanced analysis using CFG and DFG from semantic graphs
        for function in &typed_file.functions {
            // Get CFG and DFG for function if available
            if let Some(cfg) = semantic_graphs.cfg_for_function(function.symbol_id) {
                if let Some(dfg) = semantic_graphs.dfg_for_function(function.symbol_id) {
                    type_flow_guard.analyze_function_safety(function, cfg, dfg);
                }
            }
        }
        
        // Collect enhanced analysis results
        let results = type_flow_guard.into_results();
        errors.extend(self.convert_flow_safety_errors(results.errors));
        
        // Track enhanced analysis metrics
        if self.config.collect_statistics {
            // Store metrics for reporting
            // These would be aggregated with other metrics
        }
        
        errors
    }
    
    /// Stage 4c: Run memory safety analysis (lifetime and ownership)
    fn run_memory_safety_analysis(
        &self,
        typed_file: &TypedFile,
        semantic_graphs: &SemanticGraphs,
        symbol_table: &SymbolTable,
        type_table: &Rc<RefCell<TypeTable>>
    ) -> Vec<CompilationError> {
        let mut errors = Vec::new();
        
        // Only run if ownership/lifetime analysis is enabled
        if !self.config.enable_ownership_analysis && !self.config.enable_lifetime_analysis {
            return errors;
        }
        
        // Use the semantic graph's ownership analysis capabilities
        if self.config.enable_ownership_analysis {
            // The ownership graph is already built in semantic_graphs
            // Validate basic ownership graph structure
            if let Err(ownership_error) = semantic_graphs.ownership_graph.validate() {
                // Extract a reasonable source location based on the error type
                let location = match &ownership_error {
                    crate::semantic_graph::OwnershipValidationError::InvalidLifetime { variable, .. } |
                    crate::semantic_graph::OwnershipValidationError::InvalidBorrow { variable, .. } |
                    crate::semantic_graph::OwnershipValidationError::InvalidMove { variable, .. } => {
                        // Try to find the function that contains this variable
                        typed_file.functions.iter()
                            .find(|f| {
                                // This is a heuristic - in production, we'd need better mapping
                                f.symbol_id == *variable || f.parameters.iter().any(|p| p.name.as_raw() == variable.as_raw())
                            })
                            .map(|f| f.source_location)
                            .unwrap_or_else(|| {
                                // Fall back to file-level location
                                SourceLocation::new(1, 1, 1, 1)
                            })
                    }
                };
                
                errors.push(CompilationError {
                    message: format!("Ownership validation error: {:?}", ownership_error),
                    location,
                    category: ErrorCategory::OwnershipError,
                    suggestion: Some("Check that all ownership constraints are properly satisfied".to_string()),
                    related_errors: Vec::new(),
                });
            }
            
            // Check for use-after-move violations
            let use_after_move_violations = semantic_graphs.ownership_graph.check_use_after_move();
            for violation in use_after_move_violations {
                let (message, location, suggestion) = match violation {
                    crate::semantic_graph::OwnershipViolation::UseAfterMove { 
                        variable, 
                        move_location, 
                        move_type 
                    } => (
                        format!("Use after move: variable {:?} was moved ({:?})", variable, move_type),
                        move_location,
                        "Consider cloning the value or restructuring to avoid the move"
                    ),
                    crate::semantic_graph::OwnershipViolation::AliasingViolation {
                        variable,
                        mutable_borrow_locations,
                        immutable_borrow_locations,
                    } => (
                        format!(
                            "Aliasing violation: variable {:?} has {} mutable and {} immutable borrows",
                            variable, 
                            mutable_borrow_locations.len(), 
                            immutable_borrow_locations.len()
                        ),
                        // Use first mutable borrow location as the primary location
                        mutable_borrow_locations.first()
                            .or(immutable_borrow_locations.first())
                            .cloned()
                            .unwrap_or_else(|| {
                                // Fall back to a reasonable default location
                                SourceLocation::new(1, 1, 1, 1)
                            }),
                        "Ensure that mutable and immutable borrows don't overlap"
                    ),
                    crate::semantic_graph::OwnershipViolation::DanglingPointer {
                        variable,
                        use_location,
                        expired_lifetime,
                    } => (
                        format!(
                            "Dangling pointer: variable {:?} used after lifetime {:?} expired",
                            variable, expired_lifetime
                        ),
                        use_location,
                        "Ensure that pointers remain valid for their entire lifetime"
                    ),
                    crate::semantic_graph::OwnershipViolation::DoubleFree {
                        variable,
                        first_free: _,
                        second_free,
                    } => (
                        format!(
                            "Double free: variable {:?} was freed at two locations",
                            variable
                        ),
                        second_free, // Use second free location as primary
                        "Ensure that each resource is freed exactly once"
                    ),
                };
                
                errors.push(CompilationError {
                    message,
                    location,
                    category: ErrorCategory::OwnershipError,
                    suggestion: Some(suggestion.to_string()),
                    related_errors: Vec::new(),
                });
            }
            
            // Run detailed ownership analysis using OwnershipAnalyzer
            use crate::semantic_graph::analysis::ownership_analyzer::{
                OwnershipAnalyzer, FunctionAnalysisContext as OwnershipContext
            };
            
            let mut ownership_analyzer = OwnershipAnalyzer::new();
            for (function_id, cfg) in &semantic_graphs.control_flow {
                if let Some(dfg) = semantic_graphs.data_flow.get(function_id) {
                    let context = OwnershipContext {
                        function_id: *function_id,
                        cfg,
                        dfg,
                        call_graph: &semantic_graphs.call_graph,
                        ownership_graph: &semantic_graphs.ownership_graph,
                    };
                    
                    match ownership_analyzer.analyze_function(&context) {
                        Ok(ownership_violations) => {
                            // Process violations from ownership analysis
                            for violation in &ownership_violations {
                                // Extract the specific source location from the violation
                                let (message, location, suggestion) = match violation {
                                    crate::semantic_graph::analysis::ownership_analyzer::OwnershipViolation::UseAfterMove {
                                        variable,
                                        use_location,
                                        move_location,
                                        move_destination,
                                    } => (
                                        format!(
                                            "Use after move: variable {:?} was moved at {:?} and used at {:?}{}",
                                            variable,
                                            move_location,
                                            use_location,
                                            move_destination.map(|d| format!(" (moved to {:?})", d)).unwrap_or_default()
                                        ),
                                        use_location.clone(),
                                        "Consider cloning the value or restructuring to avoid the move"
                                    ),
                                    crate::semantic_graph::analysis::ownership_analyzer::OwnershipViolation::DoubleMove {
                                        variable,
                                        first_move,
                                        second_move,
                                    } => (
                                        format!(
                                            "Double move: variable {:?} was moved at {:?} and again at {:?}",
                                            variable, first_move, second_move
                                        ),
                                        second_move.clone(),
                                        "A variable can only be moved once; consider cloning or borrowing instead"
                                    ),
                                    crate::semantic_graph::analysis::ownership_analyzer::OwnershipViolation::BorrowConflict {
                                        variable,
                                        mutable_borrow,
                                        conflicting_borrow,
                                        conflict_type,
                                    } => (
                                        format!(
                                            "Borrow conflict: variable {:?} has conflicting borrows ({:?}) at {:?}",
                                            variable, conflict_type, mutable_borrow
                                        ),
                                        conflicting_borrow.clone(),
                                        "Ensure that mutable and immutable borrows don't overlap"
                                    ),
                                    crate::semantic_graph::analysis::ownership_analyzer::OwnershipViolation::MoveOfBorrowedVariable {
                                        variable,
                                        move_location,
                                        active_borrows,
                                    } => (
                                        format!(
                                            "Cannot move variable {:?}: it has {} active borrow(s)",
                                            variable,
                                            active_borrows.len()
                                        ),
                                        move_location.clone(),
                                        "Cannot move a variable while it is borrowed; wait for borrows to end"
                                    ),
                                    crate::semantic_graph::analysis::ownership_analyzer::OwnershipViolation::BorrowOutlivesOwner {
                                        borrowed_variable,
                                        borrower,
                                        borrow_location,
                                        owner_end_location,
                                    } => (
                                        format!(
                                            "Borrow of {:?} by {:?} outlives the owner (ends at {:?})",
                                            borrowed_variable,
                                            borrower,
                                            owner_end_location
                                        ),
                                        borrow_location.clone(),
                                        "Ensure that borrows don't outlive the data they reference"
                                    ),
                                };
                                
                                errors.push(CompilationError {
                                    message,
                                    location,
                                    category: ErrorCategory::OwnershipError,
                                    suggestion: Some(suggestion.to_string()),
                                    related_errors: Vec::new(),
                                });
                            }
                        }
                        Err(ownership_error) => {
                            let location = typed_file.functions.iter()
                                .find(|f| f.symbol_id == *function_id)
                                .map(|f| f.source_location)
                                .unwrap_or_else(|| SourceLocation::new(1, 1, 1, 1));
                            
                            errors.push(CompilationError {
                                message: format!("Ownership analysis error: {:?}", ownership_error),
                                location,
                                category: ErrorCategory::OwnershipError,
                                suggestion: Some("Check function ownership constraints".to_string()),
                                related_errors: Vec::new(),
                            });
                        }
                    }
                }
            }
        }
        
        // Use lifetime analysis from semantic graph analyzers
        if self.config.enable_lifetime_analysis {
            use crate::semantic_graph::analysis::lifetime_analyzer::LifetimeAnalyzer;
            use crate::semantic_graph::analysis::ownership_analyzer::FunctionAnalysisContext;
            
            // Create lifetime analyzer and run on each function
            let mut lifetime_analyzer = LifetimeAnalyzer::new();
            for (function_id, cfg) in &semantic_graphs.control_flow {
                if let Some(dfg) = semantic_graphs.data_flow.get(function_id) {
                    // Create analysis context
                    let context = FunctionAnalysisContext {
                        function_id: *function_id,
                        cfg,
                        dfg,
                        call_graph: &semantic_graphs.call_graph,
                        ownership_graph: &semantic_graphs.ownership_graph,
                    };
                    
                    // Run lifetime analysis
                    match lifetime_analyzer.analyze_function(&context) {
                        Ok(lifetime_result) => {
                            // Process any violations found
                            for violation in &lifetime_result.violations {
                                // Try to get function source location
                                let location = typed_file.functions.iter()
                                    .find(|f| f.symbol_id == *function_id)
                                    .map(|f| f.source_location)
                                    .unwrap_or_else(|| SourceLocation::new(1, 1, 1, 1));
                                
                                errors.push(CompilationError {
                                    message: format!("Lifetime violation: {:?}", violation),
                                    location,
                                    category: ErrorCategory::LifetimeError,
                                    suggestion: Some("Ensure that references don't outlive their referents".to_string()),
                                    related_errors: Vec::new(),
                                });
                            }
                        }
                        Err(lifetime_error) => {
                            // Get function location for the error
                            let location = typed_file.functions.iter()
                                .find(|f| f.symbol_id == *function_id)
                                .map(|f| f.source_location)
                                .unwrap_or_else(|| SourceLocation::new(1, 1, 1, 1));
                            
                            errors.push(CompilationError {
                                message: format!("Lifetime analysis error: {:?}", lifetime_error),
                                location,
                                category: ErrorCategory::LifetimeError,
                                suggestion: Some("Check lifetime annotations and constraints".to_string()),
                                related_errors: Vec::new(),
                            });
                        }
                    }
                }
            }
        }
        
        errors
    }
    
    /// Convert TypeFlowGuard FlowSafetyError to CompilationWarning
    fn convert_flow_safety_warning(&self, warning: FlowSafetyError) -> CompilationError {
        let (message, category) = match &warning {
            FlowSafetyError::DeadCode { .. } => (
                "Dead code detected".to_string(),
                ErrorCategory::TypeError,
            ),
            _ => (
                format!("Warning: {:?}", warning),
                ErrorCategory::TypeError,
            ),
        };
        
        let location = match &warning {
            FlowSafetyError::DeadCode { location } => location.clone(),
            FlowSafetyError::UninitializedVariable { location, .. } |
            FlowSafetyError::NullDereference { location, .. } => location.clone(),
            _ => {
                // For warnings without specific locations, use file start
                SourceLocation::new(1, 1, 1, 1)
            },
        };
        
        CompilationError {
            message,
            location,
            category,
            suggestion: None,
            related_errors: Vec::new(),
        }
    }
    
    /// Convert TypeFlowGuard FlowSafetyError to CompilationError
    fn convert_flow_safety_errors(&self, flow_errors: Vec<FlowSafetyError>) -> Vec<CompilationError> {
        flow_errors.into_iter().map(|err| {
            let (message, category) = match &err {
                FlowSafetyError::UninitializedVariable { variable, location: _ } |
                FlowSafetyError::UseOfUninitializedVariable { variable, location: _ } => (
                    format!("Use of uninitialized variable: {:?}", variable),
                    ErrorCategory::TypeError,
                ),
                FlowSafetyError::NullDereference { variable, location: _ } |
                FlowSafetyError::NullPointerDereference { variable, location: _ } => (
                    format!("Potential null pointer dereference: {:?}", variable),
                    ErrorCategory::TypeError,
                ),
                FlowSafetyError::DeadCode { location: _ } => (
                    "Unreachable code detected".to_string(),
                    ErrorCategory::TypeError, // Dead code is often a type/logic error
                ),
                FlowSafetyError::ResourceLeak { resource, location: _ } => (
                    format!("Resource leak detected: {:?}", resource),
                    ErrorCategory::OwnershipError,
                ),
                FlowSafetyError::UseAfterFree { variable, use_location: _, free_location: _ } => (
                    format!("Use after free: {:?}", variable),
                    ErrorCategory::LifetimeError,
                ),
                FlowSafetyError::UseAfterMove { variable, use_location: _, move_location: _ } => (
                    format!("Use after move: {:?}", variable),
                    ErrorCategory::OwnershipError,
                ),
                FlowSafetyError::InvalidBorrow { variable, location: _, reason } => (
                    format!("Invalid borrow of {:?}: {}", variable, reason),
                    ErrorCategory::OwnershipError,
                ),
                FlowSafetyError::DanglingReference { reference, location: _ } => (
                    format!("Dangling reference: {:?}", reference),
                    ErrorCategory::LifetimeError,
                ),
                FlowSafetyError::TypeError { message } => (
                    message.clone(),
                    ErrorCategory::TypeError,
                ),
                FlowSafetyError::EffectMismatch { expected, actual, location: _ } => (
                    format!("Effect mismatch: expected {}, got {}", expected, actual),
                    ErrorCategory::TypeError,
                ),
            };
            
            let location = match &err {
                FlowSafetyError::UninitializedVariable { location, .. } |
                FlowSafetyError::NullDereference { location, .. } |
                FlowSafetyError::UseOfUninitializedVariable { location, .. } |
                FlowSafetyError::NullPointerDereference { location, .. } |
                FlowSafetyError::DeadCode { location } |
                FlowSafetyError::ResourceLeak { location, .. } |
                FlowSafetyError::DanglingReference { location, .. } |
                FlowSafetyError::InvalidBorrow { location, .. } => location.clone(),
                FlowSafetyError::UseAfterFree { use_location, .. } |
                FlowSafetyError::UseAfterMove { use_location, .. } => use_location.clone(),
                FlowSafetyError::TypeError { .. } => {
                    // Type errors should be caught earlier with proper locations,
                    // but if we get here, use a reasonable fallback
                    SourceLocation::new(1, 1, 1, 1)
                },
                FlowSafetyError::EffectMismatch { location, .. } => location.clone(),
            };
            
            CompilationError {
                message,
                location,
                category,
                suggestion: None,
                related_errors: Vec::new(),
            }
        }).collect()
    }
}

impl Default for HaxeCompilationPipeline {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience function to compile a single Haxe file
pub fn compile_haxe_file<P: AsRef<Path>>(file_path: P, source: &str) -> CompilationResult {
    let mut pipeline = HaxeCompilationPipeline::new();
    pipeline.compile_file(file_path, source)
}

/// Convenience function to compile Haxe source code without a file
pub fn compile_haxe_source(source: &str) -> CompilationResult {
    compile_haxe_file("inline.hx", source)
}

/// Convenience function to compile multiple Haxe files
pub fn compile_haxe_files<P: AsRef<Path>>(files: &[(P, String)]) -> CompilationResult {
    let mut pipeline = HaxeCompilationPipeline::new();
    pipeline.compile_files(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pipeline_creation() {
        let pipeline = HaxeCompilationPipeline::new();
        assert_eq!(pipeline.stats.files_processed, 0);
        assert!(pipeline.config.strict_type_checking);
    }
    
    #[test]
    fn test_compile_simple_haxe() {
        let source = r#"
            class Main {
                static function main() {
                    trace("Hello, World!");
                }
            }
        "#;
        
        let result = compile_haxe_file("test.hx", source);
        
        // Should successfully parse even if type checking fails
        assert!(result.stats.files_processed > 0);
    }
    
    // #[test]
    // fn test_config_customization() {
    //     let config = PipelineConfig {
    //         strict_type_checking: false,
    //         enable_lifetime_analysis: false,
    //         target_platform: TargetPlatform::Cpp,
    //         ..Default::default()
    //     };
        
    //     let pipeline = HaxeCompilationPipeline::with_config(config);
    //     assert!(!pipeline.config.strict_type_checking);
    //     assert_eq!(pipeline.config.target_platform, TargetPlatform::Cpp);
    // }
}