# ADR-0002: Bytecode Wrapper Method Renaming for Clean HotSpot Verification

## Status
Accepted

## Date
2026-10-18

## Context
A mod loader must hook critical lifecycle methods in the vanilla Minecraft game, including server startup (`DedicatedServer.initServer`), game loop tick (`MinecraftServer.tickServer`), and registry finalization (`BuiltInRegistries.freeze`).

In modern Java runtimes (Java 7+ up to modern Java 25), JVM bytecode verification strictly enforces the presence and validity of the `StackMapTable` attribute (RFC / JVM Specification §4.7.4). If instructions or branching logic are inserted directly into an existing method body without recalculating every stack map frame, the JVM immediately terminates with a fatal `java.lang.VerifyError`.

Recalculating a valid `StackMapTable` in native Rust at runtime requires:
1. Full control flow graph (CFG) analysis across basic blocks.
2. Complete class hierarchy knowledge to compute the Least Common Ancestor (LCA) of operand stack object types at control flow merge points.
3. Access to all referenced classes via classloader lookups, which risks triggering class loading recursion or deadlocks during `ClassFileLoadHook`.

## Decision
Adopt the **Wrapper Method Renaming Technique** for all bytecode hooks in `redox-mc-loader`:
1. Rename the original vanilla target method from `<name>` to `<name>$original` with private or identical visibility.
2. Synthesize a new replacement method retaining the original method name, descriptor, and access flags.
3. Generate a straight-line basic block for the wrapper:
   - Call pre-hook via `invokestatic net/redoxmc/bridge/RedoxBridge.on<HookName>(...)`.
   - Push method receiver (`aload_0` if non-static) and all arguments onto the operand stack (`aload`, `iload`, `lload`, `fload`, `dload`).
   - Invoke the renamed original method (`invokevirtual`, `invokestatic`, or `invokespecial` to `<name>$original`).
   - If non-void, stash the return value into a local variable (`astore`, `istore`, etc.).
   - Call post-hook via `invokestatic net/redoxmc/bridge/RedoxBridge.on<HookName>(...)`.
   - Reload return value (if non-void) and execute matching return opcode (`return`, `areturn`, `ireturn`, `lreturn`, `freturn`, `dreturn`).
4. Because the synthesized wrapper contains zero branch instructions (`goto`, `if_*`, `tableswitch`, `lookupswitch`) and zero exception handlers, it has exactly one basic block. Under the JVM specification, methods with a single basic block require **no `StackMapTable` attribute**.

## Alternatives Considered

### In-Place Bytecode Injection with StackMapTable Recalculation
- **Pros:** Does not introduce an extra synthetic method in the class definition.
- **Cons:** Requires a full JVM bytecode analyzer in native Rust capable of class hierarchy traversal and LCA calculation. Any missing class or obfuscation edge case causes a fatal `VerifyError`.
- **Rejected:** Excessive complexity and runtime fragility.

### JVMTI Breakpoint / Method Entry Events
- **Pros:** No bytecode transformation needed.
- **Cons:** Triggers VM deoptimization, forces methods into interpreted mode, and adds severe per-call overhead (orders of magnitude slower than compiled code).
- **Rejected:** Unacceptable performance degradation for high-frequency methods like `tickServer()`.

### Native JNI Detours / Inline Machine Code Hooks
- **Pros:** Completely bypasses Java bytecode.
- **Cons:** Unstable across HotSpot JIT (C1/C2) compiler tiers, breaks inlining and on-stack replacement (OSR), and platform-dependent (x86_64 vs aarch64 machine code patching).
- **Rejected:** Unsafe and unmaintainable across architectures.

## Consequences
- **Guaranteed Bytecode Verifier Compliance:** 100% immune to `VerifyError` on Java 25.
- **No Classloader Deadlocks:** Transformation is entirely self-contained; no class hierarchy lookups or recursive class loading during `ClassFileLoadHook`.
- **HotSpot JIT Friendly:** Modern HotSpot JIT (C1/C2) trivially inlines trivial wrapper methods, achieving zero-cost abstraction after warmup.
- **Stack Traces:** The original method appears in stack traces as `<name>$original()`, providing clear visibility that a method has been wrapped.
