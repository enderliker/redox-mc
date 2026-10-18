# RustMC

A native Rust modloader for Minecraft Java Edition.

RustMC allows writing Minecraft mods in 100% native Rust, compiled directly to native shared libraries (`.so`, `.dll`, `.dylib`) rather than running as `.jar` bytecode on the JVM. The modloader attaches to the JVM via a high-performance JVMTI agent and orchestrates mod loading, registry hooks, and native events with zero overhead.

## Project Status

- **Current Phase:** Phase 1 (Foundation and Vertical Slice).
- **Completed Tasks:**
  - **Task 1.1:** Core JVMTI agent in Rust (`rustmc-loader`).
  - **Task 1.2:** Three-tier Java bridge architecture (`bridge-core`, `bridge-game`, `bridge-client`), isolated `URLClassLoader` capture, and safe `DefineClass` outside hooks.
  - **Task 1.3:** Decoupled version adapter architecture (`rustmc-adapter-api`, `rustmc-adapter-26-3`), canonical hook table, and bytecode validation tests.

## Compilation and Testing

### Prerequisites
- JDK 25 (`javac`, `jar`, `javap`)
- Rust toolchain (`cargo`, `rustc`)

### Build
```bash
# 1. Compile Java bridge modules
./rustmc-bridge/build.sh

# 2. Build Rust workspace
cargo build --workspace

# 3. Run adapter tests against Minecraft bytecode
cargo test --workspace

# 4. Run full agent integration tests
./tests/run_task_1_2_tests.sh
```

## Legal Disclaimer

This is an unofficial, community-driven project not affiliated with, endorsed by, or associated with Mojang Studios or Microsoft Corporation. This repository contains and distributes **no proprietary Minecraft code, assets, or binaries**. Users must supply their own legally purchased copy of Minecraft to run and test mods.

## License

Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
