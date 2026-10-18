# ADR-0003: Stable C-ABI Layout and Symmetric Memory Ownership Across Dynamic Libraries

## Status
Accepted

## Date
2026-10-18

## Context
In Redox-MC, both the host agent (`redox-mc-loader`) and third-party mods are compiled as separate dynamic shared libraries (`.so` on Linux, `.dll` on Windows, `.dylib` on macOS).

Dynamic library boundaries in Rust introduce three critical pitfalls:
1. **Unstable Rust ABI:** Rust does not have a stable ABI. Compiling a mod with a slightly different `rustc` version, profile (`debug` vs `release`), or compiler flags can result in incompatible struct layouts, vtable alignments, and calling conventions.
2. **Allocator Mismatch & Heap Segregation:** Each dynamically loaded library may instantiate its own memory allocator (e.g. `jemalloc`, `mimalloc`, or system allocator). Furthermore, on Windows with MSVC, different DLLs may bind to distinct C Runtime (CRT) heaps. Allocating memory in one library (e.g. a mod) and freeing it in another (e.g. the host loader) causes allocator metadata corruption, heap crashes, and undefined behavior.
3. **Data Structure Misalignment:** Unspecified struct layouts can introduce architecture-specific alignment padding differences.

## Decision
1. **Strict `#[repr(C)]` Contracts in `redox-mc-abi`:**
   - Every structure, enum, and function pointer crossing the FFI boundary is defined in `redox-mc-abi` with `#[repr(C)]`.
   - Structs with pointers or 64-bit integers include explicit padding fields where necessary to guarantee identical 64-bit alignment across all supported architectures.
   - All entry points and callbacks use `extern "C"` calling convention.
   - An explicit ABI version handshake (`REDOX_ABI_VERSION`) is verified during mod initialization.
2. **Symmetric Memory Ownership Pattern:**
   - The entity that allocates memory owns its deallocation. Memory is never freed across the dynamic library boundary by the opposing party's allocator.
   - Host-allocated memory is accompanied by host deallocation functions (`redox_loader_free_*` exposed via `RedoxHostVTable`).
   - Mod-allocated memory is freed either within the mod before returning from a function call or via mod-provided destructor function pointers (`redox_mod_free_*`).
   - Types like `FfiString` and `FfiBuffer` support non-owning borrows (`as_str()`, `as_slice()`) whenever possible to avoid unnecessary allocations across the FFI boundary.

## Alternatives Considered

### Relying on Rust `dylib` Dynamic Linking
- **Pros:** Native Rust types without `#[repr(C)]` wrappers.
- **Cons:** Strictly requires every mod and the host to be compiled with the exact same rustc compiler version, same target triple, and compatible compiler flags. Any toolchain discrepancy causes silent memory corruption.
- **Rejected:** Completely unfeasible for a modding ecosystem where mods are distributed precompiled by independent developers.

### Forcing libc `malloc` / `free` via Global Allocator
- **Pros:** Standardizes on system C allocator.
- **Cons:** On Windows, different MSVC CRT runtimes (e.g. `/MD` vs `/MT` or debug vs release) maintain distinct process heaps; freeing a block allocated by one CRT in another CRT causes instant heap assertion failure. Mod developers would also be constrained from using optimized allocators.
- **Rejected:** Unreliable on Windows and limits mod developer flexibility.

### WebAssembly (Wasm) Sandbox
- **Pros:** Absolute memory isolation; portable bytecode.
- **Cons:** High overhead for high-frequency game loop interactions (e.g. tick events, entity updates, rendering pipeline); awkward integration with JNI and direct native JVM pointers.
- **Rejected:** Fails the primary goal of zero-cost native execution.

## Consequences
- **True Binary Compatibility:** Mods compiled with any toolchain conforming to the C-ABI specification can link with the loader.
- **Zero Allocator Aliasing:** Heap corruption and CRT mismatch crashes are systematically eliminated.
- **Platform Agnostic:** Works consistently across Linux (ELF), Windows (PE/COFF), and macOS (Mach-O).
- **Explicit Ownership:** Developers must be conscious of memory lifetime and deallocation responsibility at the boundary.
