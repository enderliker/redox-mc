# JNI Boundary Crossing Overhead & Performance Characterization

## 1. Executive Summary

As part of **Task 2.4** in the Redox-MC architecture, we implemented a microbenchmarking harness (`redox-mc-loader --bin jni-bench`) to measure the precise latency overhead of JNI transitions between Rust (native code) and HotSpot JVM (Java 25).

The benchmark measures:
1. **Llamadas sin argumentos (No-arg call)**: Baseline JNI boundary transition.
2. **Paso de primitivos (Primitive arguments)**: JNI call with primitive scalar arguments (`jlong`, `jvalue`).
3. **Strings pequeños (Small strings)**: Native UTF-8 conversion, JVM `jstring` heap allocation, method inquiry (`String.length()`), and local reference cleanup (`DeleteLocalRef`).
4. **Objetos complejos (Complex objects)**: Full JVM object instantiation (`NewObjectA`), virtual method dispatch (`StringBuilder.append`), state inquiry (`length()`), and reference cleanup.

---

## 2. Benchmark Environment & Methodology

- **Host Environment:** Linux x86_64 (`6.6.137+rpt-rpi-2712` / amd64 container)
- **JVM Runtime:** OpenJDK 64-Bit Server VM (build 25-ea+14-Ubuntu-1, mixed mode, sharing)
- **Compiler:** `rustc 1.99.0-nightly`, compiled with `--release` (opt-level 3, LTO enabled)
- **Warmup:** 50,000 iterations per suite to ensure HotSpot TieredCompilation (C1 and C2 JIT) compiles invocation stubs and JNI trampolines.
- **Sample Design:** Batched measurement ($B = 1,000$ iterations per batch, $N = 200$ batches = 200,000 measured iterations total) to eliminate clock query Heisenberg distortion (~15-20 ns per `Instant::now()` query).
- **Statistics:** Exact percentiles (p50, p90, p95, p99), arithmetic mean, min, max, and standard deviation.

---

## 3. Empirical Results

```
==========================================================================================
                REDOX-MC: JNI BOUNDARY CROSSING BENCHMARK REPORT
                Environment: Java 25 HotSpot JVM (Linux x86_64)
==========================================================================================
Benchmark Category                  |      Avg |      p50 |      p90 |      p99 | Ref (10-25ns)
------------------------------------------------------------------------------------------
1. No-arg Call (System.nanoTime)    |  96.34 ns |  89.00 ns | 118.11 ns | 161.92 ns |   MODERATE
2. Primitive Args (Math.max(jlon... |  83.48 ns |  77.60 ns |  93.65 ns | 142.75 ns |   MODERATE
3. Small String (NewStringUTF + ... | 215.10 ns | 190.80 ns | 272.31 ns | 388.38 ns |  HIGH COST
4. Complex Object (NewObject + a... | 469.23 ns | 352.55 ns | 460.76 ns | 715.25 ns |  HIGH COST
==========================================================================================
```

### Detailed Breakdown

| Metric | 1. No-arg Call | 2. Primitive Args | 3. Small String | 4. Complex Object |
| :--- | :---: | :---: | :---: | :---: |
| **Average (Mean)** | 96.34 ns | 83.48 ns | 215.10 ns | 469.23 ns |
| **p50 (Median)** | 89.00 ns | 77.60 ns | 190.80 ns | 352.55 ns |
| **p90** | 118.11 ns | 93.65 ns | 272.31 ns | 460.76 ns |
| **p99** | 161.92 ns | 142.75 ns | 388.38 ns | 715.25 ns |
| **Overhead Multiplier** | 1.0x | 0.9x | **~2.5x** | **~4.5x - 8x** |

---

## 4. Architectural Analysis & HotSpot Mechanics

### 4.1 Why JNI Transitions Cost ~75-90 ns
When native Rust code invokes a Java method via JNI (`CallStaticLongMethodA`, `CallObjectMethodA`):
1. **Thread State Transition:** HotSpot must switch the calling thread's state from `_thread_in_native` to `_thread_in_vm` and then `_thread_in_Java`. This requires atomic memory barriers (`lock cmpxchg` or seq_cst fences) so the Garbage Collector safepoint mechanism can track thread states.
2. **Argument Unpacking:** Pointers in `jvalue*` arrays must be copied into machine registers (ABI calling convention translation between C calling convention `SysV AMD64` and HotSpot internal calling convention).
3. **Exception Checking & Frame Setup:** Setting up Java frame anchors (`JavaFrameAnchor`) so stack walking and stack trace capture can traverse between native and Java frames.
4. **Return Transition:** Re-flipping thread state back to `_thread_in_native` upon method completion.

### 4.2 Why Strings & Objects Escalate Costs (>200-450 ns)
- **`NewStringUTF`**: Requires scanning UTF-8 bytes, allocating a Java `byte[]` / `java.lang.String` on the Java heap, adding a reference entry in the JNI local reference table (`JNIHandleBlock`), and running compact string validation.
- **`NewObjectA`**: Triggers dynamic object allocation on the TLAB (Thread-Local Allocation Buffer), constructor dispatch, and reference tracking.
- **`DeleteLocalRef`**: Mutates the thread's local reference table. Without explicit deletion or frame management (`PushLocalFrame`/`PopLocalFrame`), the table leaks memory and degrades lookup speeds.

---

## 5. Validation of Redox-MC's Declarative Design Directive

In Minecraft, the game engine targets **20 Ticks Per Second (TPS)**, giving a strict budget of **50.0 ms per tick**:
$$\text{Tick Budget} = 50,000,000 \text{ ns}$$

If a mod executes naive, fine-grained JNI queries in common game loops (e.g., iterating 100,000 blocks or entities during a tick):
- $100,000 \times 200 \text{ ns} = 20,000,000 \text{ ns} = \mathbf{20.0 \text{ ms}}$!
- **40% of the entire tick budget would be consumed purely by JNI context-switching overhead**, resulting in severe tick lag and dropped TPS.

### Conclusion & Design Enforcements for Redox-MC
1. **Zero Chatty JNI in Tick Loops:** Mod logic must never call individual JNI accessors per block, entity, or coordinate in game loops.
2. **Coarse-Grained Batching:** Bridge hooks must batch updates into flat arrays or contiguous buffers.
3. **Shared Memory (`FfiBuffer` / `repr(C)`):** Data crossing the boundary should be transferred through contiguous zero-copy buffers allocated symmetrically in Rust/C memory (`redox_loader_free_*` / `redox_mod_free_*`).
4. **Declarative Mod Registration:** Registrations of blocks, items, hooks, and event listeners must happen declaratively during bootstrap (`onBootstrapEntry` / `onRegisterContent`) using bulk arrays, completely eliminating runtime reflection and JNI thrashing.
