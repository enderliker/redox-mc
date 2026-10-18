# ADR-0004: In-Mod Panic Isolation Boundary and HotSpot Signal Non-Interference

## Status
Accepted

## Date
2026-10-18

## Context
When executing native Rust mod code embedded inside the Minecraft JVM process:
1. **Unwinding Across FFI is Undefined Behavior:** In standard Rust, panics trigger stack unwinding. Unwinding across an `extern "C"` boundary is undefined behavior; the Rust runtime terminates the entire process with an unrecoverable abort.
2. **Crash Containment:** A bug or assertion failure in a single third-party mod should not crash the entire Minecraft server or corrupt JVM state.
3. **HotSpot VM Signal Handlers:** The HotSpot JVM relies heavily on OS-level signals (`SIGSEGV` and `SIGBUS` on Linux/macOS, Structured Exception Handling / SEH on Windows) for regular execution semantics, including:
   - Implicit null-pointer checks (HotSpot emits null-dereference instructions without branch checks, relying on `SIGSEGV` traps to branch into exception handlers).
   - JIT safepoint polling and deoptimization.
   - Guard page stack overflow detection.
   If native Rust code installs global signal handlers (such as custom `sigaction` or Windows VEH) to intercept crashes, it intercepts HotSpot's internal signals, destabilizing or crashing the JVM.

## Decision
1. **Panic Containment at the Mod Export Boundary:**
   - Every mod entry point and exported function is annotated with `#[redox_export]` (provided by `redox-mc-api-macros`).
   - The macro automatically wraps the inner Rust implementation in `std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| { ... }))`.
   - If a panic occurs:
     - The panic payload is caught inside the mod's dynamic library.
     - A formatted panic diagnostic is logged to the error stream.
     - A safe fallback value or error code (`RedoxStatus::Error`) is returned across the `extern "C"` boundary via the `PanicFallback` trait.
   - No Rust panic ever unwinds across `extern "C"`.
2. **Zero Signal Handler Registration Policy:**
   - As enforced in `redox-mc-loader/src/safety.rs`, the loader and mods **never** register custom signal handlers for `SIGSEGV`, `SIGBUS`, `SIGFPE`, `SIGILL`, or Windows VEH.
   - The HotSpot JVM retains exclusive ownership of hardware exception traps.
   - Rust code remains compliant with JVM signal chaining (`libjsig`).

## Alternatives Considered

### Process Isolation (Out-of-Process Mods via Shared Memory / IPC)
- **Pros:** Absolute isolation: a mod crash or segfault kills only its own process.
- **Cons:** High IPC serialization/context switch overhead for every game tick and event callback; makes direct memory mapping and pointer exchange impossible.
- **Rejected:** Unacceptable latency for real-time game ticking and high-throughput events.

### Installing Custom OS Signal Handlers (`sigaction` / SEH)
- **Pros:** Catches low-level segfaults in native C/Rust code.
- **Cons:** Catches HotSpot's implicit null checks and safepoint traps, causing immediate deadlocks, false positives, or JVM crashes unless complex chaining through `libjsig` is replicated.
- **Rejected:** Dangerous anti-pattern in JVM-embedded native environments.

### Relying on `panic = "abort"`
- **Pros:** Zero unwinding overhead.
- **Cons:** Any `panic!` or `unwrap()` failure immediately kills the server process, offering zero chance for error logging, clean mod unloading, or saving game state.
- **Rejected:** Incompatible with fault tolerance.

## Consequences
- **Total Panic Safety:** Panics in mod code are safely contained and logged without triggering UB or JVM aborts.
- **HotSpot Stability:** JVM JIT optimizations (implicit null checks, safepoints) continue operating at peak performance without interference.
- **Developer Ergonomics:** Mod authors write idiomatic Rust using `Result` or standard assertions; `#[redox_export]` handles the ABI wrapping boilerplate transparently.
