//! RPKG Builder — constructs `.rpkg` archives from components.
//!
//! Packages can be pure Haxe (library classes only), native (extern classes +
//! dylib), or mixed (extern classes, library classes that wrap them, and a
//! dylib). The builder accepts any combination of entries.

use super::{EntryKind, EntryMeta, MethodDescEntry, RpkgEntry, RpkgToc, RPKG_MAGIC, RPKG_VERSION};
use std::path::Path;

/// Accumulates entries and writes the final `.rpkg` archive.
pub struct RpkgBuilder {
    package_name: String,
    /// (kind, meta, raw bytes)
    entries: Vec<(EntryKind, EntryMeta, Vec<u8>)>,
}

impl RpkgBuilder {
    pub fn new(package_name: &str) -> Self {
        RpkgBuilder {
            package_name: package_name.to_string(),
            entries: Vec::new(),
        }
    }

    /// Add a native library for a specific platform.
    pub fn add_native_lib(&mut self, data: &[u8], os: &str, arch: &str) {
        self.entries.push((
            EntryKind::NativeLib,
            EntryMeta::NativeLib {
                os: os.to_string(),
                arch: arch.to_string(),
            },
            data.to_vec(),
        ));
    }

    /// Add a native library from a file path.
    pub fn add_native_lib_from_file(
        &mut self,
        path: &Path,
        os: &str,
        arch: &str,
    ) -> Result<(), std::io::Error> {
        let data = std::fs::read(path)?;
        self.add_native_lib(&data, os, arch);
        Ok(())
    }

    /// Add a Haxe source file for extern class declarations.
    pub fn add_haxe_source(&mut self, module_path: &str, source: &str) {
        self.entries.push((
            EntryKind::HaxeSource,
            EntryMeta::HaxeSource {
                module_path: module_path.to_string(),
            },
            source.as_bytes().to_vec(),
        ));
    }

    /// Add a serialized method table.
    pub fn add_method_table(&mut self, plugin_name: &str, methods: &[MethodDescEntry]) {
        let data = postcard::to_allocvec(methods).expect("method table serialization failed");
        self.entries.push((
            EntryKind::MethodTable,
            EntryMeta::MethodTable {
                plugin_name: plugin_name.to_string(),
            },
            data,
        ));
    }

    /// Write the complete `.rpkg` archive to disk.
    ///
    /// Layout: [entry data...][TOC (postcard)][toc_size: u32][version: u32][magic: 4]
    pub fn write(&self, output: &Path) -> Result<(), super::RpkgError> {
        use std::io::Write;

        let mut file = std::fs::File::create(output)?;
        let mut toc_entries = Vec::with_capacity(self.entries.len());
        let mut offset: u64 = 0;

        // Write entry data and build TOC
        for (kind, meta, data) in &self.entries {
            file.write_all(data)?;
            toc_entries.push(RpkgEntry {
                kind: *kind,
                offset,
                size: data.len() as u64,
                meta: meta.clone(),
            });
            offset += data.len() as u64;
        }

        // Serialize and write TOC
        let toc = RpkgToc {
            package_name: self.package_name.clone(),
            entries: toc_entries,
        };
        let toc_bytes =
            postcard::to_allocvec(&toc).map_err(super::RpkgError::DeserializationFailed)?;
        let toc_size = toc_bytes.len() as u32;
        file.write_all(&toc_bytes)?;

        // Write footer: toc_size, version, magic
        file.write_all(&toc_size.to_le_bytes())?;
        file.write_all(&RPKG_VERSION.to_le_bytes())?;
        file.write_all(RPKG_MAGIC)?;

        Ok(())
    }
}

/// Build an `.rpkg` from a compiled native dylib and a directory of `.hx` files.
///
/// This convenience function:
/// 1. Reads the dylib and adds it as a NativeLib for the current platform
/// 2. Loads method descriptors from the dylib's `plugin_describe()` export
/// 3. Collects all `.hx` files from `haxe_dir` as HaxeSource entries
/// 4. Writes the final `.rpkg`
pub fn build_from_dylib(
    package_name: &str,
    dylib_path: &Path,
    haxe_dir: &Path,
    output: &Path,
) -> Result<(), String> {
    let mut builder = RpkgBuilder::new(package_name);

    // 1. Add native lib for current platform
    let os = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        return Err("unsupported OS".to_string());
    };
    let arch = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else {
        return Err("unsupported architecture".to_string());
    };

    builder
        .add_native_lib_from_file(dylib_path, os, arch)
        .map_err(|e| format!("failed to read dylib: {}", e))?;

    // 2. Load method descriptors from dylib
    let methods = extract_method_table_from_dylib(dylib_path)?;
    if !methods.is_empty() {
        builder.add_method_table(package_name, &methods);
    }

    // 3. Collect .hx files
    if haxe_dir.is_dir() {
        collect_haxe_sources(&mut builder, haxe_dir, haxe_dir)?;
    }

    // 4. Write
    builder
        .write(output)
        .map_err(|e| format!("failed to write rpkg: {}", e))?;

    Ok(())
}

/// Build a pure-Haxe `.rpkg` from a directory of `.hx` files (no native lib).
pub fn build_from_haxe_dir(
    package_name: &str,
    haxe_dir: &Path,
    output: &Path,
) -> Result<(), String> {
    let mut builder = RpkgBuilder::new(package_name);

    if haxe_dir.is_dir() {
        collect_haxe_sources(&mut builder, haxe_dir, haxe_dir)?;
    } else {
        return Err(format!("{} is not a directory", haxe_dir.display()));
    }

    builder
        .write(output)
        .map_err(|e| format!("failed to write rpkg: {}", e))?;

    Ok(())
}

/// Walk a directory tree and add all `.hx` files as HaxeSource entries.
fn collect_haxe_sources(
    builder: &mut RpkgBuilder,
    base_dir: &Path,
    current_dir: &Path,
) -> Result<(), String> {
    let entries = std::fs::read_dir(current_dir)
        .map_err(|e| format!("failed to read dir {}: {}", current_dir.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("dir entry error: {}", e))?;
        let path = entry.path();

        if path.is_dir() {
            collect_haxe_sources(builder, base_dir, &path)?;
        } else if path.extension().map(|e| e == "hx").unwrap_or(false) {
            let rel_path = path
                .strip_prefix(base_dir)
                .map_err(|e| format!("strip_prefix failed: {}", e))?;
            let module_path = rel_path.to_string_lossy().to_string();
            let source = std::fs::read_to_string(&path)
                .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;
            builder.add_haxe_source(&module_path, &source);
        }
    }

    Ok(())
}

/// Load the dylib, call its `plugin_describe()` export, and convert to MethodDescEntry.
fn extract_method_table_from_dylib(dylib_path: &Path) -> Result<Vec<MethodDescEntry>, String> {
    type DescribeFn = unsafe extern "C" fn(*mut usize) -> *const rayzor_plugin::NativeMethodDesc;

    let lib = unsafe { libloading::Library::new(dylib_path) }
        .map_err(|e| format!("failed to load dylib {}: {}", dylib_path.display(), e))?;

    // Try common describe function names
    let describe_names = [
        b"rayzor_gpu_plugin_describe" as &[u8],
        b"plugin_describe",
        b"rayzor_plugin_describe",
    ];

    for name in &describe_names {
        if let Ok(describe_fn) = unsafe { lib.get::<DescribeFn>(name) } {
            let mut count: usize = 0;
            let descs = unsafe { describe_fn(&mut count) };
            if descs.is_null() || count == 0 {
                continue;
            }

            let slice = unsafe { std::slice::from_raw_parts(descs, count) };
            let mut methods = Vec::with_capacity(count);

            for desc in slice {
                let symbol_name = unsafe {
                    std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                        desc.symbol_name,
                        desc.symbol_name_len,
                    ))
                    .to_string()
                };
                let class_name = unsafe {
                    std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                        desc.class_name,
                        desc.class_name_len,
                    ))
                    .to_string()
                };
                let method_name = unsafe {
                    std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                        desc.method_name,
                        desc.method_name_len,
                    ))
                    .to_string()
                };
                let param_types = desc.param_types[..desc.param_count as usize].to_vec();

                methods.push(MethodDescEntry {
                    symbol_name,
                    class_name,
                    method_name,
                    is_static: desc.is_static != 0,
                    param_count: desc.param_count,
                    return_type: desc.return_type,
                    param_types,
                });
            }

            // Keep library alive until we're done reading (it's on stack, will be dropped)
            // The data is now owned strings, so dropping lib is safe.
            return Ok(methods);
        }
    }

    // No describe function found — that's OK, might be a plain dylib
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_empty_package() {
        let builder = RpkgBuilder::new("empty");
        let tmp = std::env::temp_dir().join("test_empty.rpkg");
        builder.write(&tmp).expect("write failed");

        let loaded = super::super::load_rpkg(&tmp).expect("load failed");
        assert_eq!(loaded.package_name, "empty");
        assert!(loaded.methods.is_empty());
        assert!(loaded.haxe_sources.is_empty());
        assert!(loaded.native_lib_bytes.is_none());

        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn builder_multiple_platforms() {
        let mut builder = RpkgBuilder::new("multi-platform");
        builder.add_native_lib(b"macos-arm", "macos", "aarch64");
        builder.add_native_lib(b"linux-x64", "linux", "x86_64");

        let tmp = std::env::temp_dir().join("test_multi_platform.rpkg");
        builder.write(&tmp).expect("write failed");

        let loaded = super::super::load_rpkg(&tmp).expect("load failed");

        // Should pick the matching platform
        if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
            assert_eq!(
                loaded.native_lib_bytes.as_deref(),
                Some(b"macos-arm" as &[u8])
            );
        } else if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
            assert_eq!(
                loaded.native_lib_bytes.as_deref(),
                Some(b"linux-x64" as &[u8])
            );
        }

        std::fs::remove_file(&tmp).ok();
    }
}
