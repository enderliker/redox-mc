# Contributing to Redox-MC

Thank you for your interest in contributing to Redox-MC! This project aims to bring native, memory-safe Rust modding to Minecraft Java Edition with zero compromise on stability, performance, and JVM compatibility.

Please review this guide before submitting issues, creating pull requests, or proposing architectural changes.

---

## Code of Conduct

We are committed to providing a welcoming, inclusive, and harassment-free environment for everyone. Please be respectful, constructive, and collaborative in all discussions and code reviews.

---

## Development Prerequisites

- **Rust:** Stable toolchain (1.85+ recommended). Install via [rustup](https://rustup.rs).
- **Java Development Kit (JDK):** JDK 25 with `javac`, `jar`, and `javap` available on your `$PATH`.
- **Git:** Version 2.30+.
- **Operating System:** Linux (x86_64, aarch64), Windows (x86_64), or macOS (aarch64).

---

## Getting Started

1. **Fork and Clone the Repository:**
   ```bash
   git clone https://github.com/your-username/rustmc.git
   cd rustmc
   ```

2. **Compile Java Bridge Modules:**
   ```bash
   ./redox-mc-bridge/build.sh
   ```

3. **Verify the Rust Workspace:**
   ```bash
   cargo build --workspace --all-targets
   cargo test --workspace
   ```

4. **Verify Lints and Formatting:**
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   ```

---

## Development Workflow & Standards

### 1. Test-Driven Development (TDD)

We practice Test-Driven Development:
1. **Red:** Write a failing unit or integration test demonstrating the bug or new feature requirement.
2. **Green:** Implement the minimal code needed to pass the test.
3. **Refactor:** Clean up the implementation while keeping all tests passing.

### 2. Architectural Invariants

Before writing code that touches JNI, JVMTI, bytecode, or dynamic library boundaries, read **[AGENTS.md](AGENTS.md)** and the relevant **[Architecture Decision Records](docs/decisions/)**:
- **Never register OS signal handlers** (`SIGSEGV`, `SIGBUS`, etc.) in native code — HotSpot relies on them for implicit null checks and deoptimization.
- **Never perform in-place bytecode surgery** that invalidates `StackMapTable` frames — use the wrapper method renaming pattern ([ADR-0002](docs/decisions/0002-bytecode-wrapper-hook-technique.md)).
- **Never allocate in one library and free in another** across dynamic library boundaries — adhere to symmetric deallocation ([ADR-0003](docs/decisions/0003-c-abi-layout-and-symmetric-memory-ownership.md)).
- **Never allow panics to unwind across `extern "C"`** — use `#[redox_export]` ([ADR-0004](docs/decisions/0004-panic-isolation-boundary.md)).

### 3. Commit Message Conventions

We follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <short description in imperative mood>

[optional body explaining motivation and context]

[optional footer(s), e.g. Closes #123]
```

**Allowed Types:**
- `feat`: A new feature or capability.
- `fix`: A bug fix.
- `docs`: Documentation additions or revisions.
- `refactor`: Code change that neither fixes a bug nor adds a feature.
- `perf`: A code change that improves performance.
- `test`: Adding or correcting tests.
- `chore`: Maintenance tasks, CI updates, dependencies.

**Example:**
```
feat(api): add priority ordering and cancellation to EventBus

Introduces EventPriority (Highest, Normal, Lowest) and CancellableEvent
trait so that security and interceptor mods execute deterministically
before standard gameplay handlers.

Closes #42
```

---

## Making Architectural Changes (ADRs)

If your contribution introduces:
- A new major dependency or framework,
- A change to the public C-ABI layout or memory ownership model,
- A new bytecode manipulation strategy, or
- A significant architectural boundary,

You **must** submit an Architecture Decision Record (ADR) under `docs/decisions/` following the format outlined in [ADR-0001](docs/decisions/0001-jvmti-agent-and-classloader-tiering.md).

---

## Pull Request Checklist

Before opening or requesting review on a Pull Request, verify:

- [ ] All Java bridge jars build cleanly (`./redox-mc-bridge/build.sh`).
- [ ] All unit and integration tests pass (`cargo test --workspace`).
- [ ] Linter passes with zero warnings (`cargo clippy --workspace --all-targets -- -D warnings`).
- [ ] Code is formatted (`cargo fmt --all -- --check`).
- [ ] No compiler warnings are silenced with `#![allow(...)]` without strong documented rationale.
- [ ] Documentation and comments explain *why* non-obvious choices were made.
- [ ] An ADR is included if introducing or modifying architectural decisions.
- [ ] No proprietary Minecraft classes, assets, or jars are included.

Thank you for helping build the future of native Minecraft modding!
