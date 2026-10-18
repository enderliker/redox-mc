# Security Policy

Redox-MC operates at the lowest layers of the process hierarchy, integrating native compiled Rust code with the HotSpot Java Virtual Machine (JVM) through JVMTI and JNI. Because native shared libraries execute within the same process address space as the JVM, maintaining memory safety, boundary isolation, and system stability is paramount.

---

## Supported Versions

| Version | Supported | Notes |
|---|---|---|
| `main` (active development) | :white_check_mark: | Active development branch |
| Phase 1 / Phase 2 releases | :white_check_mark: | Security patches backported if critical |
| Legacy / Experimental branches | :x: | Unsupported |

---

## Security Architecture & Invariants

Redox-MC enforces strict low-level security contracts to protect against memory corruption, denial-of-service, and process instability:

### 1. Dynamic Library Memory Boundary
- **Symmetric Allocator Ownership:** Dynamic libraries (`.so`, `.dll`, `.dylib`) linked to different allocators must never free memory allocated by another library. The `redox-mc-abi` crate provides paired allocator/deallocator function pointers (`redox_loader_free_*` and `redox_mod_free_*`). Violations of this ownership rule are treated as critical security defects.
- **Strict `#[repr(C)]` Layouts:** All data crossing the FFI boundary uses explicitly aligned C-compatible representations with fixed 64-bit padding to eliminate structure misalignment vulnerabilities across compiler toolchains.

### 2. Panic Isolation
- **No Unwinding Across FFI:** Rust unwinding across `extern "C"` functions is undefined behavior. All exported mod entry points are guarded by `#[redox_export]`, which catches panics using `catch_unwind(AssertUnwindSafe)` and returns safe error status codes (`RedoxStatus::Error`), preventing process aborts and heap inconsistency.

### 3. HotSpot Signal Non-Interference
- **Hardware Trap Preservation:** The HotSpot JVM relies on OS-level hardware fault signals (`SIGSEGV`, `SIGBUS`, and Windows SEH) to perform implicit null-pointer checks, JIT deoptimization safepoints, and stack overflow handling. Redox-MC strictly prohibits registering custom OS signal handlers that could intercept or disrupt these mechanisms.

### 4. Input Validation & Deserialization Safety
- **Zip / Jar Extraction Safety:** Tooling such as `redox-mc-bindgen` validates all zip archive entries against directory traversal attacks ("Zip Slip") and enforces bounds checking on constant pool indexes and bytecode offsets.
- **Mod Loading Validation:** Dynamic libraries are discovered only from designated mod directories and validated for required ABI version symbols before linking.

---

## Reporting a Vulnerability

We take the security of Redox-MC and its users seriously. If you discover a security vulnerability, please report it responsibly rather than opening a public issue.

### How to Report

Please report security issues via one of the following methods:
1. **GitHub Security Advisories:** Navigate to the repository's **Security** tab and click **"Report a vulnerability"** (preferred).
2. **Security Contact:** Email the maintainers directly at `security@redoxmc.org` (or maintainer contact listed in repository settings) with:
   - A detailed description of the vulnerability.
   - Steps to reproduce or a minimal proof-of-concept (PoC).
   - The affected operating system, architecture, and JVM version.
   - Potential impact (e.g. arbitrary code execution, denial of service, memory corruption).

### Our Commitment

- We will acknowledge receipt of your vulnerability report within **48 hours**.
- We will provide an initial assessment and estimated remediation timeline within **5 business days**.
- We will coordinate public disclosure with you after a patch has been developed, tested, and released.
- We will credit you in the security advisory (unless you prefer to remain anonymous).
