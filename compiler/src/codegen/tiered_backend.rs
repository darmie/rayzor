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

use super::cranelift_backend::CraneliftBackend;
use super::profiling::{ProfileData, ProfileConfig, ProfileStatistics};
use crate::ir::{IrFunction, IrFunctionId, IrModule};

#[cfg(feature = "llvm-backend")]
use super::llvm_jit_backend::LLVMJitBackend;
#[cfg(feature = "llvm-backend")]
use inkwell::context::Context;
use tracing::debug;

/// Tiered compilation backend
pub struct TieredBackend {
    /// Primary Cranelift backend (used for Tier 0 compilation)
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

    /// The MIR module (needed for recompilation)
    module: Arc<RwLock<Option<IrModule>>>,

    /// Configuration
    config: TieredConfig,

    /// Background optimization worker handle
    worker_handle: Option<thread::JoinHandle<()>>,

    /// Shutdown signal for background worker
    shutdown: Arc<Mutex<bool>>,
}

/// Optimization tier level (4-tier system)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OptimizationTier {
    Baseline,     // Tier 0: Cranelift, fast compilation, minimal optimization
    Standard,     // Tier 1: Cranelift, moderate optimization
    Optimized,    // Tier 2: Cranelift, aggressive optimization
    Maximum,      // Tier 3: LLVM, maximum optimization for ultra-hot code
}

impl OptimizationTier {
    /// Get Cranelift optimization level for this tier (T0-T2 only)
    pub fn cranelift_opt_level(&self) -> &'static str {
        match self {
            OptimizationTier::Baseline => "none",              // T0: No optimization
            OptimizationTier::Standard => "speed",             // T1: Moderate
            OptimizationTier::Optimized => "speed_and_size",   // T2: Aggressive
            OptimizationTier::Maximum => "speed_and_size",     // T3 uses LLVM, not Cranelift
        }
    }

    /// Check if this tier uses LLVM backend
    pub fn uses_llvm(&self) -> bool {
        matches!(self, OptimizationTier::Maximum)
    }

    /// Get the next higher tier (if any)
    pub fn next_tier(&self) -> Option<OptimizationTier> {
        match self {
            OptimizationTier::Baseline => Some(OptimizationTier::Standard),
            OptimizationTier::Standard => Some(OptimizationTier::Optimized),
            OptimizationTier::Optimized => Some(OptimizationTier::Maximum),
            OptimizationTier::Maximum => None, // Already at max
        }
    }

    /// Get a human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            OptimizationTier::Baseline => "Baseline (T0/Cranelift)",
            OptimizationTier::Standard => "Standard (T1/Cranelift)",
            OptimizationTier::Optimized => "Optimized (T2/Cranelift)",
            OptimizationTier::Maximum => "Maximum (T3/LLVM)",
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
}

impl Default for TieredConfig {
    fn default() -> Self {
        Self {
            profile_config: ProfileConfig::default(),
            enable_background_optimization: true,
            optimization_check_interval_ms: 100,
            max_parallel_optimizations: 4,
            verbosity: 0,
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
        }
    }
}

impl TieredBackend {
    /// Create a new tiered backend
    pub fn new(config: TieredConfig) -> Result<Self, String> {
        let baseline_backend = CraneliftBackend::new()?;
        let profile_data = ProfileData::new(config.profile_config);

        Ok(Self {
            baseline_backend: Arc::new(Mutex::new(baseline_backend)),
            profile_data,
            function_tiers: Arc::new(RwLock::new(HashMap::new())),
            function_pointers: Arc::new(RwLock::new(HashMap::new())),
            optimization_queue: Arc::new(Mutex::new(VecDeque::new())),
            optimizing: Arc::new(Mutex::new(HashSet::new())),
            module: Arc::new(RwLock::new(None)),
            config,
            worker_handle: None,
            shutdown: Arc::new(Mutex::new(false)),
        })
    }

    /// Compile a MIR module (initially at Tier 0 - Baseline)
    pub fn compile_module(&mut self, module: IrModule) -> Result<(), String> {
        if self.config.verbosity >= 1 {
            debug!(
                "[TieredBackend] Compiling {} functions at Tier 0 (Baseline)",
                module.functions.len()
            );
        }

        // Compile everything at baseline (Tier 0)
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

        // Store module for later recompilation
        *self.module.write().unwrap() = Some(module);

        // Start background optimization if enabled
        if self.config.enable_background_optimization {
            self.start_background_optimization();
        }

        Ok(())
    }

    /// Get a function pointer (for execution)
    pub fn get_function_pointer(&self, func_id: IrFunctionId) -> Option<*const u8> {
        self.function_pointers
            .read()
            .unwrap()
            .get(&func_id)
            .map(|addr| *addr as *const u8)
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

        // Check if function should be promoted to next tier
        let should_promote = {
            let tiers = self.function_tiers.read().unwrap();
            let current_tier = tiers
                .get(&func_id)
                .copied()
                .unwrap_or(OptimizationTier::Baseline);

            match current_tier {
                OptimizationTier::Baseline if self.profile_data.is_warm(func_id) => {
                    Some(OptimizationTier::Standard)
                }
                OptimizationTier::Standard if self.profile_data.is_hot(func_id) => {
                    Some(OptimizationTier::Optimized)
                }
                OptimizationTier::Optimized if self.profile_data.is_blazing(func_id) => {
                    Some(OptimizationTier::Maximum)  // Promote to LLVM!
                }
                _ => None,
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
            if self.config.verbosity >= 2 {
                let count = self.profile_data.get_function_count(func_id);
                debug!(
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

        // Get the function from the module
        let module_lock = self.module.read().unwrap();
        let module = module_lock
            .as_ref()
            .ok_or_else(|| "Module not loaded".to_string())?;

        let function = module
            .functions
            .get(&func_id)
            .ok_or_else(|| format!("Function {:?} not found", func_id))?;

        // Choose backend based on tier
        let new_ptr = if target_tier.uses_llvm() {
            // Tier 3: Use LLVM backend
            self.compile_with_llvm(func_id, function)?
        } else {
            // Tier 0-2: Use Cranelift backend
            self.compile_with_cranelift(func_id, function, target_tier)?
        };

        // Drop the module lock before updating pointers
        drop(module_lock);

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
        function: &IrFunction,
        target_tier: OptimizationTier,
    ) -> Result<usize, String> {
        // Create a new Cranelift backend with the target optimization level
        let mut backend = CraneliftBackend::with_optimization_level(target_tier.cranelift_opt_level())?;

        // Get the module
        let module_lock = self.module.read().unwrap();
        let module = module_lock.as_ref().ok_or("Module not set")?;

        // Compile the function at the new optimization level
        backend.compile_single_function(func_id, module, function)?;

        // Get the optimized function pointer
        let ptr = backend.get_function_ptr(func_id)?;
        Ok(ptr as usize)
    }

    /// Compile function with LLVM backend (Tier 3)
    #[cfg(feature = "llvm-backend")]
    fn compile_with_llvm(
        &self,
        func_id: IrFunctionId,
        function: &IrFunction,
    ) -> Result<usize, String> {
        // Create LLVM context and backend
        let context = Context::create();
        let mut backend = LLVMJitBackend::new(&context)?;

        // Compile the function with maximum LLVM optimization
        backend.compile_single_function(func_id, function)?;

        // Get the optimized function pointer
        let ptr = backend.get_function_ptr(func_id)?;
        Ok(ptr as usize)
    }

    /// Compile function with LLVM backend (Tier 3) - stub when LLVM not enabled
    #[cfg(not(feature = "llvm-backend"))]
    fn compile_with_llvm(
        &self,
        func_id: IrFunctionId,
        _function: &IrFunction,
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
        let module = Arc::clone(&self.module);
        let function_pointers = Arc::clone(&self.function_pointers);
        let function_tiers = Arc::clone(&self.function_tiers);
        let shutdown = Arc::clone(&self.shutdown);
        let profile_data = self.profile_data.clone();
        let config = self.config.clone();

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
                    &module,
                    &function_pointers,
                    &function_tiers,
                    &profile_data,
                    &config,
                );

                // Sleep before next iteration
                thread::sleep(Duration::from_millis(config.optimization_check_interval_ms));
            }
        });

        self.worker_handle = Some(handle);
    }

    /// Background worker iteration (processes one function from queue)
    fn background_worker_iteration(
        queue: &Arc<Mutex<VecDeque<(IrFunctionId, OptimizationTier)>>>,
        optimizing: &Arc<Mutex<HashSet<IrFunctionId>>>,
        module: &Arc<RwLock<Option<IrModule>>>,
        function_pointers: &Arc<RwLock<HashMap<IrFunctionId, usize>>>,
        function_tiers: &Arc<RwLock<HashMap<IrFunctionId, OptimizationTier>>>,
        profile_data: &ProfileData,
        config: &TieredConfig,
    ) {
        let mut queue_lock = queue.lock().unwrap();
        let mut optimizing_lock = optimizing.lock().unwrap();

        // Don't start new optimizations if at capacity
        if optimizing_lock.len() >= config.max_parallel_optimizations {
            return;
        }

        // Dequeue a function to optimize
        if let Some((func_id, target_tier)) = queue_lock.pop_front() {
            optimizing_lock.insert(func_id);
            drop(queue_lock);
            drop(optimizing_lock);

            // Perform optimization
            let result = Self::worker_optimize_function(
                func_id,
                target_tier,
                module,
                function_pointers,
                function_tiers,
                profile_data,
                config,
            );

            // Mark as done
            optimizing.lock().unwrap().remove(&func_id);

            if let Err(e) = result {
                if config.verbosity >= 1 {
                    debug!("[TieredBackend] Failed to optimize {:?}: {}", func_id, e);
                }
            }
        }
    }

    /// Worker function to optimize a single function (called by background thread)
    fn worker_optimize_function(
        func_id: IrFunctionId,
        target_tier: OptimizationTier,
        module: &Arc<RwLock<Option<IrModule>>>,
        function_pointers: &Arc<RwLock<HashMap<IrFunctionId, usize>>>,
        function_tiers: &Arc<RwLock<HashMap<IrFunctionId, OptimizationTier>>>,
        profile_data: &ProfileData,
        config: &TieredConfig,
    ) -> Result<(), String> {
        if config.verbosity >= 1 {
            let count = profile_data.get_function_count(func_id);
            debug!(
                "[TieredBackend] Worker optimizing {:?} at {} (count: {})",
                func_id,
                target_tier.description(),
                count
            );
        }

        // Get the function from the module
        let module_lock = module.read().unwrap();
        let module_ref = module_lock
            .as_ref()
            .ok_or_else(|| "Module not loaded".to_string())?;

        let function = module_ref
            .functions
            .get(&func_id)
            .ok_or_else(|| format!("Function {:?} not found", func_id))?;

        // Compile with appropriate backend
        let new_ptr = if target_tier.uses_llvm() {
            // Tier 3: Use LLVM backend
            Self::compile_with_llvm_static(func_id, module_ref, function)?
        } else {
            // Tier 0-2: Use Cranelift backend
            Self::compile_with_cranelift_static(func_id, module_ref, function, target_tier)?
        };

        // Drop the module lock before updating pointers
        drop(module_lock);

        // Atomically swap the function pointer
        function_pointers
            .write()
            .unwrap()
            .insert(func_id, new_ptr);
        function_tiers
            .write()
            .unwrap()
            .insert(func_id, target_tier);

        if config.verbosity >= 1 {
            debug!(
                "[TieredBackend] Worker successfully recompiled {:?} at {}",
                func_id,
                target_tier.description()
            );
        }

        Ok(())
    }

    /// Static version of compile_with_cranelift for use in worker thread
    fn compile_with_cranelift_static(
        func_id: IrFunctionId,
        module: &IrModule,
        function: &IrFunction,
        target_tier: OptimizationTier,
    ) -> Result<usize, String> {
        let mut backend = CraneliftBackend::with_optimization_level(target_tier.cranelift_opt_level())?;
        backend.compile_single_function(func_id, module, function)?;
        let ptr = backend.get_function_ptr(func_id)?;
        Ok(ptr as usize)
    }

    /// Static version of compile_with_llvm for use in worker thread
    ///
    /// Note: This intentionally leaks the LLVM context and backend to ensure
    /// JIT-compiled code remains valid for the program's lifetime.
    #[cfg(feature = "llvm-backend")]
    fn compile_with_llvm_static(
        func_id: IrFunctionId,
        _module: &IrModule,
        function: &IrFunction,
    ) -> Result<usize, String> {
        // Create context and backend, then leak them to ensure lifetime
        // This is intentional: JIT code must remain valid indefinitely
        let context = Box::leak(Box::new(Context::create()));
        let mut backend = LLVMJitBackend::new(context)?;
        // TODO: LLVM backend also needs to accept module parameter
        backend.compile_single_function(func_id, function)?;
        let ptr = backend.get_function_ptr(func_id)?;

        // Leak the backend to keep the execution engine alive
        Box::leak(Box::new(backend));

        Ok(ptr as usize)
    }

    /// Static version of compile_with_llvm - stub when LLVM not enabled
    #[cfg(not(feature = "llvm-backend"))]
    fn compile_with_llvm_static(
        func_id: IrFunctionId,
        _module: &IrModule,
        _function: &IrFunction,
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
            "Tiered Compilation: {} Baseline (T0), {} Standard (T1), {} Optimized (T2)\n\
             Queue: {} waiting, {} optimizing\n\
             {}",
            self.baseline_functions,
            self.standard_functions,
            self.optimized_functions,
            self.queued_for_optimization,
            self.currently_optimizing,
            self.profile_stats.format()
        )
    }
}
