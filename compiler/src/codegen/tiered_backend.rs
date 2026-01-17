//! # Tiered Compilation Backend
//!
//! Implements multi-tier JIT compilation using Cranelift with different optimization levels.
//! Automatically recompiles hot functions with higher optimization based on runtime profiling.
//!
//! ## Optimization Tiers
//! - **Tier 0 (Baseline)**: Minimal optimization, fastest compilation (for cold code)
//! - **Tier 1 (Standard)**: Moderate optimization (for warm code)
//! - **Tier 2 (Optimized)**: Aggressive optimization (for hot code)
//!
//! ## How It Works
//! 1. All functions start at Tier 0 (baseline JIT)
//! 2. Execution counters track how often functions are called
//! 3. When a function crosses the "warm" threshold, it's recompiled at Tier 1
//! 4. When it crosses the "hot" threshold, it's recompiled at Tier 2
//! 5. Function pointers are atomically swapped after recompilation
//!
//! ## Architecture
//! - Main thread: Executes code, records profile data
//! - Background worker: Monitors hot functions, performs async recompilation
//! - Lock-free atomic counters: Minimal overhead profiling
//! - RwLock for function pointer map: Fast reads, infrequent writes

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

use rayon::prelude::*;

use super::cranelift_backend::CraneliftBackend;
use super::mir_interpreter::{MirInterpreter, InterpValue, InterpError};
use super::profiling::{ProfileData, ProfileConfig, ProfileStatistics};
use crate::ir::{IrFunction, IrFunctionId, IrModule};

#[cfg(feature = "llvm-backend")]
use super::llvm_jit_backend::LLVMJitBackend;
#[cfg(feature = "llvm-backend")]
use inkwell::context::Context;
use tracing::debug;

/// Tiered compilation backend
pub struct TieredBackend {
    /// MIR interpreter for Phase 0 (instant startup)
    interpreter: Arc<Mutex<MirInterpreter>>,

    /// Primary Cranelift backend (used for Phase 1+ compilation)
    baseline_backend: Arc<Mutex<CraneliftBackend>>,

    /// Runtime profiling data
    profile_data: ProfileData,

    /// Current optimization tier for each function
    function_tiers: Arc<RwLock<HashMap<IrFunctionId, OptimizationTier>>>,

    /// Function pointers (usize for thread safety, cast to function type when needed)
    function_pointers: Arc<RwLock<HashMap<IrFunctionId, usize>>>,

    /// Queue of functions waiting for recompilation at higher tier
    optimization_queue: Arc<Mutex<VecDeque<(IrFunctionId, OptimizationTier)>>>,

    /// Functions currently being optimized (prevents duplicate work)
    optimizing: Arc<Mutex<HashSet<IrFunctionId>>>,

    /// The MIR modules (needed for recompilation and interpretation)
    /// Multiple modules may be loaded (e.g., user code + stdlib)
    modules: Arc<RwLock<Vec<IrModule>>>,

    /// Configuration
    config: TieredConfig,

    /// Background optimization worker handle
    worker_handle: Option<thread::JoinHandle<()>>,

    /// Shutdown signal for background worker
    shutdown: Arc<Mutex<bool>>,

    /// Whether to start in interpreted mode (Phase 0)
    start_interpreted: bool,

    /// Runtime symbols for FFI (used by interpreter and LLVM backend)
    /// Stored as (name, pointer) pairs for thread-safe sharing
    runtime_symbols: Arc<Vec<(String, usize)>>,
}

/// Optimization tier level (5-tier system with interpreter)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OptimizationTier {
    Interpreted,  // Phase 0: MIR interpreter (instant startup, ~5-10x native speed)
    Baseline,     // Phase 1: Cranelift, fast compilation, minimal optimization
    Standard,     // Phase 2: Cranelift, moderate optimization
    Optimized,    // Phase 3: Cranelift, aggressive optimization
    Maximum,      // Phase 4: LLVM, maximum optimization for ultra-hot code
}

impl OptimizationTier {
    /// Get Cranelift optimization level for this tier (Phase 1-3 only)
    pub fn cranelift_opt_level(&self) -> &'static str {
        match self {
            OptimizationTier::Interpreted => "none",           // P0: Not used (interpreter)
            OptimizationTier::Baseline => "none",              // P1: No optimization
            OptimizationTier::Standard => "speed",             // P2: Moderate
            // TODO: "speed_and_size" causes incorrect results in some cases (checksum halved)
            // Using "speed" until the root cause is identified
            OptimizationTier::Optimized => "speed",            // P3: Was "speed_and_size"
            OptimizationTier::Maximum => "speed",              // P4 uses LLVM, not Cranelift
        }
    }

    /// Check if this tier uses the interpreter
    pub fn uses_interpreter(&self) -> bool {
        matches!(self, OptimizationTier::Interpreted)
    }

    /// Check if this tier uses LLVM backend
    pub fn uses_llvm(&self) -> bool {
        matches!(self, OptimizationTier::Maximum)
    }

    /// Get the next higher tier (if any)
    pub fn next_tier(&self) -> Option<OptimizationTier> {
        match self {
            OptimizationTier::Interpreted => Some(OptimizationTier::Baseline),
            OptimizationTier::Baseline => Some(OptimizationTier::Standard),
            OptimizationTier::Standard => Some(OptimizationTier::Optimized),
            OptimizationTier::Optimized => Some(OptimizationTier::Maximum),
            OptimizationTier::Maximum => None, // Already at max
        }
    }

    /// Get a human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            OptimizationTier::Interpreted => "Interpreted (P0/MIR)",
            OptimizationTier::Baseline => "Baseline (P1/Cranelift)",
            OptimizationTier::Standard => "Standard (P2/Cranelift)",
            OptimizationTier::Optimized => "Optimized (P3/Cranelift)",
            OptimizationTier::Maximum => "Maximum (P4/LLVM)",
        }
    }
}

/// Configuration for tiered compilation
#[derive(Debug, Clone)]
pub struct TieredConfig {
    /// Profiling configuration
    pub profile_config: ProfileConfig,

    /// Enable background optimization (async optimization in separate thread)
    pub enable_background_optimization: bool,

    /// How often to check for hot functions (in milliseconds)
    pub optimization_check_interval_ms: u64,

    /// Maximum number of functions to optimize in parallel
    pub max_parallel_optimizations: usize,

    /// Verbosity level (0 = silent, 1 = basic, 2 = detailed)
    pub verbosity: u8,

    /// Start in interpreted mode (Phase 0) for instant startup
    /// If false, functions are compiled to Baseline (Phase 1) immediately
    pub start_interpreted: bool,
}

impl Default for TieredConfig {
    fn default() -> Self {
        Self {
            profile_config: ProfileConfig::default(),
            enable_background_optimization: true,
            optimization_check_interval_ms: 100,
            max_parallel_optimizations: 4,
            verbosity: 0,
            start_interpreted: true, // Enable interpreter by default for instant startup
        }
    }
}

impl TieredConfig {
    /// Development configuration (aggressive optimization, verbose)
    pub fn development() -> Self {
        Self {
            profile_config: ProfileConfig::development(),
            enable_background_optimization: true,
            optimization_check_interval_ms: 50,
            max_parallel_optimizations: 2,
            verbosity: 2,
            start_interpreted: true, // Instant startup for quick iteration
        }
    }

    /// Production configuration (conservative, low overhead)
    pub fn production() -> Self {
        Self {
            profile_config: ProfileConfig::production(),
            enable_background_optimization: true,
            optimization_check_interval_ms: 1000,
            max_parallel_optimizations: 8,
            verbosity: 0,
            start_interpreted: true, // Instant startup, then promote hot functions
        }
    }

    /// JIT-only configuration (skip interpreter, compile immediately)
    /// Use when startup time is less important than consistent performance
    pub fn jit_only() -> Self {
        Self {
            profile_config: ProfileConfig::default(),
            enable_background_optimization: true,
            optimization_check_interval_ms: 100,
            max_parallel_optimizations: 4,
            verbosity: 0,
            start_interpreted: false, // Skip interpreter, start at Phase 1
        }
    }
}

impl TieredBackend {
    /// Create a new tiered backend
    pub fn new(config: TieredConfig) -> Result<Self, String> {
        // IMPORTANT: Initialize LLVM on the main thread BEFORE any background workers start.
        #[cfg(feature = "llvm-backend")]
        super::llvm_jit_backend::init_llvm_once();

        let baseline_backend = CraneliftBackend::new()?;
        let profile_data = ProfileData::new(config.profile_config);
        let start_interpreted = config.start_interpreted;

        Ok(Self {
            interpreter: Arc::new(Mutex::new(MirInterpreter::new())),
            baseline_backend: Arc::new(Mutex::new(baseline_backend)),
            profile_data,
            function_tiers: Arc::new(RwLock::new(HashMap::new())),
            function_pointers: Arc::new(RwLock::new(HashMap::new())),
            optimization_queue: Arc::new(Mutex::new(VecDeque::new())),
            optimizing: Arc::new(Mutex::new(HashSet::new())),
            modules: Arc::new(RwLock::new(Vec::new())),
            config,
            worker_handle: None,
            shutdown: Arc::new(Mutex::new(false)),
            start_interpreted,
            runtime_symbols: Arc::new(Vec::new()),
        })
    }

    /// Create a new tiered backend with runtime symbols for interpreter and LLVM FFI
    pub fn with_symbols(config: TieredConfig, symbols: &[(&str, *const u8)]) -> Result<Self, String> {
        // IMPORTANT: Initialize LLVM on the main thread BEFORE any background workers start.
        // This prevents crashes from LLVM initialization racing with background compilation.
        #[cfg(feature = "llvm-backend")]
        super::llvm_jit_backend::init_llvm_once();

        let baseline_backend = CraneliftBackend::new()?;
        let profile_data = ProfileData::new(config.profile_config);
        let start_interpreted = config.start_interpreted;

        // Store symbols for later LLVM backend use
        let runtime_symbols: Vec<(String, usize)> = symbols
            .iter()
            .map(|(name, ptr)| (name.to_string(), *ptr as usize))
            .collect();

        // Create interpreter and register symbols
        let mut interp = MirInterpreter::new();
        for (name, ptr) in symbols {
            interp.register_symbol(name, *ptr);
        }

        Ok(Self {
            interpreter: Arc::new(Mutex::new(interp)),
            baseline_backend: Arc::new(Mutex::new(baseline_backend)),
            profile_data,
            function_tiers: Arc::new(RwLock::new(HashMap::new())),
            function_pointers: Arc::new(RwLock::new(HashMap::new())),
            optimization_queue: Arc::new(Mutex::new(VecDeque::new())),
            optimizing: Arc::new(Mutex::new(HashSet::new())),
            modules: Arc::new(RwLock::new(Vec::new())),
            config,
            worker_handle: None,
            shutdown: Arc::new(Mutex::new(false)),
            start_interpreted,
            runtime_symbols: Arc::new(runtime_symbols),
        })
    }

    /// Compile/load a MIR module
    ///
    /// If `start_interpreted` is true:
    /// - Functions start at Phase 0 (Interpreted) for instant startup
    /// - Background worker will JIT-compile functions as they get hot
    ///
    /// If `start_interpreted` is false:
    /// - Functions are compiled to Phase 1 (Baseline) immediately
    pub fn compile_module(&mut self, module: IrModule) -> Result<(), String> {
        let initial_tier = if self.start_interpreted {
            OptimizationTier::Interpreted
        } else {
            OptimizationTier::Baseline
        };

        if self.config.verbosity >= 1 {
            debug!(
                "[TieredBackend] Loading {} functions at {} ({})",
                module.functions.len(),
                initial_tier.description(),
                if self.start_interpreted { "instant startup" } else { "JIT compiled" }
            );
        }

        if self.start_interpreted {
            // Interpreter mode: Just register functions, no compilation needed
            // Functions start at Phase 0 (Interpreted) and will be JIT-compiled on demand
            for func_id in module.functions.keys() {
                self.function_tiers
                    .write()
                    .unwrap()
                    .insert(*func_id, OptimizationTier::Interpreted);
            }
        } else {
            // JIT mode: Compile everything at baseline (Phase 1)
            let mut backend = self.baseline_backend.lock().unwrap();
            backend.compile_module(&module)?;

            // Store function pointers and mark all as Baseline tier
            for func_id in module.functions.keys() {
                if let Ok(ptr) = backend.get_function_ptr(*func_id) {
                    self.function_pointers
                        .write()
                        .unwrap()
                        .insert(*func_id, ptr as usize);
                    self.function_tiers
                        .write()
                        .unwrap()
                        .insert(*func_id, OptimizationTier::Baseline);
                }
            }
            drop(backend);
        }

        // Store module for later recompilation/interpretation
        self.modules.write().unwrap().push(module);

        // Start background optimization if enabled
        if self.config.enable_background_optimization {
            self.start_background_optimization();
        }

        Ok(())
    }

    /// Execute a function (interpreter or JIT based on current tier)
    ///
    /// Returns the result as an InterpValue, which can be converted to native types.
    pub fn execute_function(
        &mut self,
        func_id: IrFunctionId,
        args: Vec<InterpValue>,
    ) -> Result<InterpValue, String> {
        // Record the call for profiling
        self.record_call(func_id);

        // Get current tier
        let tier = self.function_tiers
            .read()
            .unwrap()
            .get(&func_id)
            .copied()
            .unwrap_or(OptimizationTier::Interpreted);

        // Debug: print current tier
        if self.config.verbosity >= 2 {
            let count = self.profile_data.get_function_count(func_id);
            eprintln!("[TieredBackend] Executing {:?} at tier {:?} (count: {})", func_id, tier, count);
        }

        if tier.uses_interpreter() {
            // Execute via interpreter - find the module containing this function
            let modules = self.modules.read().unwrap();
            let module_ref = modules.iter()
                .find(|m| m.functions.contains_key(&func_id))
                .ok_or_else(|| format!("Function {:?} not found in any module", func_id))?;
            let mut interp = self.interpreter.lock().unwrap();
            interp.execute(module_ref, func_id, args)
                .map_err(|e| format!("Interpreter error: {}", e))
        } else {
            // JIT-compiled code - call via function pointer
            let func_ptr = self.get_function_pointer(func_id)
                .ok_or_else(|| format!("JIT function {:?} not found in function_pointers", func_id))?;

            // For functions with no args (like main), call directly
            // NOTE: Cranelift adds a hidden environment parameter (i64) to non-extern Haxe
            // functions. We must pass a null pointer for this parameter.
            // For functions with args, we'd need to marshal InterpValue -> native types
            if args.is_empty() {
                unsafe {
                    // Pass null environment pointer as required by Haxe calling convention
                    let jit_fn: extern "C" fn(i64) = std::mem::transmute(func_ptr);
                    jit_fn(0); // null environment pointer
                }
                Ok(InterpValue::Void)
            } else {
                // TODO: Implement argument marshaling for JIT calls
                // For now, fall back to interpreter for functions with args
                // This is a limitation - in practice, hot inner functions often have args
                if self.config.verbosity >= 1 {
                    debug!("[TieredBackend] JIT function with args - falling back to interpreter");
                }
                let modules = self.modules.read().unwrap();
                let module_ref = modules.iter()
                    .find(|m| m.functions.contains_key(&func_id))
                    .ok_or_else(|| format!("Function {:?} not found in any module", func_id))?;
                let mut interp = self.interpreter.lock().unwrap();
                interp.execute(module_ref, func_id, args)
                    .map_err(|e| format!("Interpreter error: {}", e))
            }
        }
    }

    /// Get a function pointer (for execution)
    pub fn get_function_pointer(&self, func_id: IrFunctionId) -> Option<*const u8> {
        self.function_pointers
            .read()
            .unwrap()
            .get(&func_id)
            .map(|addr| *addr as *const u8)
    }

    /// Get the current optimization tier for a function
    pub fn get_function_tier(&self, func_id: IrFunctionId) -> OptimizationTier {
        self.function_tiers
            .read()
            .unwrap()
            .get(&func_id)
            .copied()
            .unwrap_or(OptimizationTier::Interpreted)
    }

    /// Record a function call (for profiling and tier promotion)
    /// This should be called before executing a function
    pub fn record_call(&self, func_id: IrFunctionId) {
        // Sample based on config to reduce overhead
        let count = self.profile_data.get_function_count(func_id);
        if count % self.profile_data.config().sample_rate != 0 {
            return;
        }

        self.profile_data.record_function_call(func_id);

        // Check if function should be promoted to a higher tier
        // Use count-based promotion that allows skipping tiers if count exceeds multiple thresholds
        let should_promote = {
            let tiers = self.function_tiers.read().unwrap();
            let current_tier = tiers
                .get(&func_id)
                .copied()
                .unwrap_or(OptimizationTier::Interpreted);

            let count = self.profile_data.get_function_count(func_id);
            let config = self.profile_data.config();

            // Determine target tier based on count (allows skipping tiers)
            let target_tier = if count >= config.blazing_threshold {
                OptimizationTier::Maximum
            } else if count >= config.hot_threshold {
                OptimizationTier::Optimized
            } else if count >= config.warm_threshold {
                OptimizationTier::Standard
            } else if count >= config.interpreter_threshold {
                OptimizationTier::Baseline
            } else {
                OptimizationTier::Interpreted
            };

            // Only promote if target tier is higher than current tier
            if target_tier as u8 > current_tier as u8 {
                Some(target_tier)
            } else {
                None
            }
        };

        if let Some(target_tier) = should_promote {
            self.enqueue_for_optimization(func_id, target_tier);
        }
    }

    /// Enqueue a function for optimization at a specific tier
    fn enqueue_for_optimization(&self, func_id: IrFunctionId, target_tier: OptimizationTier) {
        let mut queue = self.optimization_queue.lock().unwrap();
        let optimizing = self.optimizing.lock().unwrap();

        // Don't enqueue if already optimizing or already in queue at this tier
        if !optimizing.contains(&func_id)
            && !queue
                .iter()
                .any(|(id, tier)| *id == func_id && *tier == target_tier)
        {
            let count = self.profile_data.get_function_count(func_id);
            if target_tier.uses_llvm() {
                eprintln!(
                    "[TieredBackend] Enqueuing {:?} for LLVM (count: {})",
                    func_id, count
                );
            } else if self.config.verbosity >= 2 {
                eprintln!(
                    "[TieredBackend] Enqueuing {:?} for {} (count: {})",
                    func_id,
                    target_tier.description(),
                    count
                );
            }
            queue.push_back((func_id, target_tier));
        }
    }

    /// Manually trigger recompilation of a function at a specific tier
    pub fn optimize_function(
        &mut self,
        func_id: IrFunctionId,
        target_tier: OptimizationTier,
    ) -> Result<(), String> {
        self.optimize_function_internal(func_id, target_tier)
    }

    /// Internal: Recompile a single function at a specific tier
    fn optimize_function_internal(
        &mut self,
        func_id: IrFunctionId,
        target_tier: OptimizationTier,
    ) -> Result<(), String> {
        if self.config.verbosity >= 1 {
            let count = self.profile_data.get_function_count(func_id);
            debug!(
                "[TieredBackend] Recompiling {:?} at {} (count: {})",
                func_id,
                target_tier.description(),
                count
            );
        }

        // Get the function from the modules
        let modules_lock = self.modules.read().unwrap();
        let (module, function) = modules_lock.iter()
            .find_map(|m| m.functions.get(&func_id).map(|f| (m, f)))
            .ok_or_else(|| format!("Function {:?} not found in any module", func_id))?;

        // Choose backend based on tier
        let new_ptr = if target_tier.uses_llvm() {
            // Tier 3: Use LLVM backend - compiles all modules
            drop(modules_lock); // Release lock before heavy work
            self.compile_with_llvm(func_id)?
        } else {
            // Tier 0-2: Use Cranelift backend
            let ptr = self.compile_with_cranelift(func_id, module, function, target_tier)?;
            drop(modules_lock);
            ptr
        };

        // Atomically swap the function pointer
        self.function_pointers
            .write()
            .unwrap()
            .insert(func_id, new_ptr);
        self.function_tiers
            .write()
            .unwrap()
            .insert(func_id, target_tier);

        if self.config.verbosity >= 1 {
            debug!(
                "[TieredBackend] Successfully recompiled {:?} at {}",
                func_id,
                target_tier.description()
            );
        }

        Ok(())
    }

    /// Compile function with Cranelift backend (Tier 0-2)
    fn compile_with_cranelift(
        &self,
        func_id: IrFunctionId,
        module: &IrModule,
        function: &IrFunction,
        target_tier: OptimizationTier,
    ) -> Result<usize, String> {
        // Create a new Cranelift backend with the target optimization level
        let mut backend = CraneliftBackend::with_optimization_level(target_tier.cranelift_opt_level())?;

        // Compile the function at the new optimization level
        backend.compile_single_function(func_id, module, function)?;

        // Get the optimized function pointer
        let ptr = backend.get_function_ptr(func_id)?;
        Ok(ptr as usize)
    }

    /// Compile function with LLVM backend (Tier 3)
    ///
    /// Note: This compiles ALL modules because functions may call other
    /// functions across modules. The function pointer for the requested function
    /// is returned.
    #[cfg(feature = "llvm-backend")]
    #[allow(dead_code)]
    fn compile_with_llvm(
        &self,
        func_id: IrFunctionId,
    ) -> Result<usize, String> {
        // Create LLVM context and backend
        let context = Context::create();

        // Convert symbols to the format LLVMJitBackend expects
        let symbols: Vec<(&str, *const u8)> = self.runtime_symbols
            .iter()
            .map(|(name, ptr)| (name.as_str(), *ptr as *const u8))
            .collect();

        let mut backend = LLVMJitBackend::with_symbols(&context, &symbols)?;

        // Compile ALL modules - functions may call across modules
        let modules_lock = self.modules.read().unwrap();
        for module in modules_lock.iter() {
            backend.compile_module(module)?;
        }
        drop(modules_lock);

        backend.finalize()?;

        // Get the optimized function pointer
        let ptr = backend.get_function_ptr(func_id)?;
        Ok(ptr as usize)
    }

    /// Compile function with LLVM backend (Tier 3) - stub when LLVM not enabled
    #[cfg(not(feature = "llvm-backend"))]
    fn compile_with_llvm(
        &self,
        func_id: IrFunctionId,
    ) -> Result<usize, String> {
        if self.config.verbosity >= 1 {
            debug!(
                "[TieredBackend] LLVM backend not enabled, cannot compile {:?} at Tier 3",
                func_id
            );
        }
        Err("LLVM backend not enabled. Compile with --features llvm-backend".to_string())
    }

    /// Start background optimization worker thread
    fn start_background_optimization(&mut self) {
        if self.worker_handle.is_some() {
            return; // Already started
        }

        let queue = Arc::clone(&self.optimization_queue);
        let optimizing = Arc::clone(&self.optimizing);
        let modules = Arc::clone(&self.modules);
        let function_pointers = Arc::clone(&self.function_pointers);
        let function_tiers = Arc::clone(&self.function_tiers);
        let shutdown = Arc::clone(&self.shutdown);
        let profile_data = self.profile_data.clone();
        let config = self.config.clone();
        let runtime_symbols = Arc::clone(&self.runtime_symbols);

        let handle = thread::spawn(move || {
            if config.verbosity >= 1 {
                debug!("[TieredBackend] Background optimization worker started");
            }

            loop {
                // Check for shutdown
                if *shutdown.lock().unwrap() {
                    if config.verbosity >= 1 {
                        debug!("[TieredBackend] Background worker shutting down");
                    }
                    break;
                }

                // Process optimization queue
                Self::background_worker_iteration(
                    &queue,
                    &optimizing,
                    &modules,
                    &function_pointers,
                    &function_tiers,
                    &profile_data,
                    &config,
                    &runtime_symbols,
                );

                // Sleep before next iteration
                thread::sleep(Duration::from_millis(config.optimization_check_interval_ms));
            }
        });

        self.worker_handle = Some(handle);
    }

    /// Background worker iteration - processes multiple functions in parallel using rayon
    ///
    /// This drains up to `max_parallel_optimizations` functions from the queue and
    /// compiles them concurrently using rayon's parallel iterators. Function pointer
    /// installation is serialized for thread safety.
    fn background_worker_iteration(
        queue: &Arc<Mutex<VecDeque<(IrFunctionId, OptimizationTier)>>>,
        optimizing: &Arc<Mutex<HashSet<IrFunctionId>>>,
        modules: &Arc<RwLock<Vec<IrModule>>>,
        function_pointers: &Arc<RwLock<HashMap<IrFunctionId, usize>>>,
        function_tiers: &Arc<RwLock<HashMap<IrFunctionId, OptimizationTier>>>,
        profile_data: &ProfileData,
        config: &TieredConfig,
        runtime_symbols: &Arc<Vec<(String, usize)>>,
    ) {
        // Drain batch of functions to compile in parallel
        let batch: Vec<(IrFunctionId, OptimizationTier)> = {
            let mut queue_lock = queue.lock().unwrap();
            let mut optimizing_lock = optimizing.lock().unwrap();

            // Calculate how many functions we can compile in parallel
            let available_slots = config.max_parallel_optimizations.saturating_sub(optimizing_lock.len());
            if available_slots == 0 {
                return;
            }

            // Drain up to available_slots functions from the queue
            let mut batch = Vec::with_capacity(available_slots);
            while batch.len() < available_slots {
                if let Some((func_id, target_tier)) = queue_lock.pop_front() {
                    optimizing_lock.insert(func_id);
                    batch.push((func_id, target_tier));
                } else {
                    break;
                }
            }
            batch
        };

        if batch.is_empty() {
            return;
        }

        // Get modules reference (read lock held during parallel compilation)
        let modules_lock = modules.read().unwrap();
        if modules_lock.is_empty() {
            // No modules, mark all as done and return
            let mut optimizing_lock = optimizing.lock().unwrap();
            for (func_id, _) in &batch {
                optimizing_lock.remove(func_id);
            }
            return;
        }

        // Separate LLVM and Cranelift compilations
        // LLVM must run sequentially (not thread-safe), Cranelift can run in parallel
        let (llvm_batch, cranelift_batch): (Vec<_>, Vec<_>) = batch
            .iter()
            .cloned()  // Clone to get owned values
            .partition(|(_, tier)| tier.uses_llvm());

        // Compile Cranelift functions in parallel
        let cranelift_results: Vec<(IrFunctionId, OptimizationTier, Result<usize, String>)> = cranelift_batch
            .par_iter()
            .map(|(func_id, target_tier)| {
                // Find the module containing this function
                let (module_ref, function) = match modules_lock.iter()
                    .find_map(|m| m.functions.get(func_id).map(|f| (m, f)))
                {
                    Some(pair) => pair,
                    None => return (*func_id, *target_tier, Err(format!("Function {:?} not found in any module", func_id))),
                };

                if config.verbosity >= 2 {
                    let count = profile_data.get_function_count(*func_id);
                    debug!(
                        "[TieredBackend] Parallel compiling {:?} at {} (count: {})",
                        func_id,
                        target_tier.description(),
                        count
                    );
                }

                // Compile with Cranelift - pass runtime symbols for extern function linking
                let result = Self::compile_with_cranelift_static(*func_id, module_ref, function, *target_tier, runtime_symbols);
                (*func_id, *target_tier, result)
            })
            .collect();

        // Compile LLVM functions sequentially (LLVM is not thread-safe for context creation)
        let llvm_results: Vec<(IrFunctionId, OptimizationTier, Result<usize, String>)> = llvm_batch
            .iter()
            .map(|(func_id, target_tier)| {
                eprintln!("[TieredBackend] LLVM compilation starting for {:?}", func_id);
                #[cfg(feature = "llvm-backend")]
                let result = {
                    // For LLVM, compile ALL modules (functions may call across modules)
                    let r = Self::compile_with_llvm_static(*func_id, &modules_lock, runtime_symbols);
                    match &r {
                        Ok(ptr) => eprintln!("[TieredBackend] LLVM compilation succeeded for {:?}, ptr={:#x}", func_id, ptr),
                        Err(e) => eprintln!("[TieredBackend] LLVM compilation FAILED for {:?}: {}", func_id, e),
                    }
                    r
                };
                #[cfg(not(feature = "llvm-backend"))]
                let result = Err("LLVM backend not enabled".to_string());

                (*func_id, *target_tier, result)
            })
            .collect();

        // Combine results
        let results: Vec<_> = cranelift_results.into_iter().chain(llvm_results).collect();

        // Drop modules lock before installing results
        drop(modules_lock);

        // Install compiled function pointers (serialized for thread safety)
        {
            let mut fp_lock = function_pointers.write().unwrap();
            let mut ft_lock = function_tiers.write().unwrap();
            let mut optimizing_lock = optimizing.lock().unwrap();

            for (func_id, target_tier, result) in results {
                optimizing_lock.remove(&func_id);

                match result {
                    Ok(ptr) => {
                        fp_lock.insert(func_id, ptr);
                        ft_lock.insert(func_id, target_tier);

                        if config.verbosity >= 1 {
                            debug!(
                                "[TieredBackend] Installed {:?} at {}",
                                func_id,
                                target_tier.description()
                            );
                        }
                    }
                    Err(e) => {
                        if config.verbosity >= 1 {
                            debug!("[TieredBackend] Failed to compile {:?}: {}", func_id, e);
                        }
                    }
                }
            }
        }
    }

    /// Static version of compile_with_cranelift for use in worker thread
    fn compile_with_cranelift_static(
        func_id: IrFunctionId,
        module: &IrModule,
        function: &IrFunction,
        target_tier: OptimizationTier,
        runtime_symbols: &Arc<Vec<(String, usize)>>,
    ) -> Result<usize, String> {
        // Convert runtime symbols to format expected by Cranelift
        let symbols: Vec<(&str, *const u8)> = runtime_symbols
            .iter()
            .map(|(name, ptr)| (name.as_str(), *ptr as *const u8))
            .collect();

        let mut backend = CraneliftBackend::with_symbols_and_opt(
            target_tier.cranelift_opt_level(),
            &symbols,
        )?;
        backend.compile_single_function(func_id, module, function)?;
        let ptr = backend.get_function_ptr(func_id)?;
        Ok(ptr as usize)
    }

    /// Static version of compile_with_llvm for use in worker thread
    ///
    /// Note: This intentionally leaks the LLVM context and backend to ensure
    /// JIT-compiled code remains valid for the program's lifetime.
    ///
    /// This compiles ALL modules because functions may call other functions
    /// across modules. The function pointer for the requested function is returned.
    #[cfg(feature = "llvm-backend")]
    fn compile_with_llvm_static(
        func_id: IrFunctionId,
        modules: &[IrModule],
        runtime_symbols: &Arc<Vec<(String, usize)>>,
    ) -> Result<usize, String> {
        // Acquire global LLVM lock - LLVM is not thread-safe
        let _llvm_guard = super::llvm_jit_backend::llvm_lock();

        // Create context and backend, then leak them to ensure lifetime
        // This is intentional: JIT code must remain valid indefinitely
        let context = Box::leak(Box::new(Context::create()));

        // Convert symbols back to the format LLVMJitBackend expects
        let symbols: Vec<(&str, *const u8)> = runtime_symbols
            .iter()
            .map(|(name, ptr)| (name.as_str(), *ptr as *const u8))
            .collect();

        let mut backend = LLVMJitBackend::with_symbols(context, &symbols)?;

        // Compile ALL modules - functions may call across modules
        for module in modules {
            backend.compile_module(module)?;
        }

        // Finalize the module to create the execution engine
        backend.finalize()?;

        // Get the function pointer for the requested function
        let ptr = backend.get_function_ptr(func_id)?;

        // Leak the backend to keep the execution engine alive
        Box::leak(Box::new(backend));

        Ok(ptr as usize)
    }

    /// Static version of compile_with_llvm - stub when LLVM not enabled
    #[cfg(not(feature = "llvm-backend"))]
    fn compile_with_llvm_static(
        func_id: IrFunctionId,
        _modules: &[IrModule],
        _runtime_symbols: &Arc<Vec<(String, usize)>>,
    ) -> Result<usize, String> {
        Err(format!(
            "LLVM backend not enabled, cannot compile {:?} at Tier 3. Compile with --features llvm-backend",
            func_id
        ))
    }

    /// Get profiling and tiering statistics
    pub fn get_statistics(&self) -> TieredStatistics {
        let profile_stats = self.profile_data.get_statistics();
        let tiers = self.function_tiers.read().unwrap();

        // Debug: Print what tiers we actually have
        if self.config.verbosity >= 2 {
            debug!("[TieredBackend] Current function tiers:");
            for (func_id, tier) in tiers.iter() {
                debug!("  {:?} -> {:?}", func_id, tier);
            }
        }

        let interpreted_count = tiers
            .values()
            .filter(|&&t| t == OptimizationTier::Interpreted)
            .count();
        let baseline_count = tiers
            .values()
            .filter(|&&t| t == OptimizationTier::Baseline)
            .count();
        let standard_count = tiers
            .values()
            .filter(|&&t| t == OptimizationTier::Standard)
            .count();
        let optimized_count = tiers
            .values()
            .filter(|&&t| t == OptimizationTier::Optimized)
            .count();
        let maximum_count = tiers
            .values()
            .filter(|&&t| t == OptimizationTier::Maximum)
            .count();

        TieredStatistics {
            profile_stats,
            interpreted_functions: interpreted_count,
            baseline_functions: baseline_count,
            standard_functions: standard_count,
            optimized_functions: optimized_count,
            llvm_functions: maximum_count,
            queued_for_optimization: self.optimization_queue.lock().unwrap().len(),
            currently_optimizing: self.optimizing.lock().unwrap().len(),
        }
    }

    /// Shutdown the tiered backend (stops background worker)
    pub fn shutdown(&mut self) {
        *self.shutdown.lock().unwrap() = true;

        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for TieredBackend {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Statistics about the tiered backend
#[derive(Debug, Clone)]
pub struct TieredStatistics {
    pub profile_stats: ProfileStatistics,
    pub interpreted_functions: usize,
    pub baseline_functions: usize,
    pub standard_functions: usize,
    pub optimized_functions: usize,
    pub llvm_functions: usize,
    pub queued_for_optimization: usize,
    pub currently_optimizing: usize,
}

impl TieredStatistics {
    /// Format as human-readable string
    pub fn format(&self) -> String {
        format!(
            "Tiered Compilation: {} Interpreted (P0), {} Baseline (P1), {} Standard (P2), {} Optimized (P3), {} LLVM (P4)\n\
             Queue: {} waiting, {} optimizing\n\
             {}",
            self.interpreted_functions,
            self.baseline_functions,
            self.standard_functions,
            self.optimized_functions,
            self.llvm_functions,
            self.queued_for_optimization,
            self.currently_optimizing,
            self.profile_stats.format()
        )
    }
}
