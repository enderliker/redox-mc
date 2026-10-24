#!/usr/bin/env python3
"""End-to-end test for Task 7.2: the `cargo redox-mc` development CLI.

The test exercises the whole documented workflow with only the CLI:

1. `cargo redox-mc new demo_mod` scaffolds a cdylib project with a mod.toml.
2. `cargo redox-mc build` compiles it and stages the artifact.
3. `cargo redox-mc run --server` builds, stages and launches a real headless
   Minecraft server with the loader agent injected, in a single command.
4. The mod's source is edited, rebuilt and hot-reloaded with `/redox reload`,
   proving the development loop works without restarting the JVM.
"""

import os
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import time


def to_native_path(path):
    if not path:
        return path
    try:
        result = subprocess.run(["cygpath", "-w", path], capture_output=True, text=True)
        if result.returncode == 0 and result.stdout.strip():
            return result.stdout.strip()
    except (FileNotFoundError, PermissionError, subprocess.SubprocessError):
        pass

    if os.name == "nt" or sys.platform == "win32":
        if len(path) >= 3 and path[0] == "/" and path[2] == "/":
            drive = path[1].upper()
            return drive + ":\\" + path[3:].replace("/", "\\")

    return os.path.abspath(path)


def get_free_port():
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
            sock.bind(("", 0))
            return sock.getsockname()[1]
    except OSError:
        import random

        return random.randint(26000, 32000)


def mod_library_name():
    if sys.platform == "win32" or os.name == "nt":
        return "demo_mod.dll"
    if sys.platform == "darwin":
        return "libdemo_mod.dylib"
    return "libdemo_mod.so"


def find_cli_binary(root_dir):
    explicit = os.environ.get("CARGO_REDOX_MC_BIN")
    if explicit and os.path.isfile(explicit):
        return explicit

    names = ["cargo-redox-mc", "cargo-redox-mc.exe"]
    search_dirs = [
        os.path.join(root_dir, "target", "debug"),
        os.path.join(root_dir, "target", "release"),
        os.path.join(root_dir, "target", "x86_64-pc-windows-msvc", "debug"),
        os.path.join(root_dir, "target", "x86_64-unknown-linux-gnu", "debug"),
        os.path.join(root_dir, "target", "aarch64-apple-darwin", "debug"),
    ]
    for directory in search_dirs:
        for name in names:
            candidate = os.path.join(directory, name)
            if os.path.isfile(candidate):
                return candidate
    return os.path.join(root_dir, "target", "debug", "cargo-redox-mc")


def find_library(base_name, root_dir):
    if sys.platform == "win32" or os.name == "nt":
        names = [f"{base_name}.dll", f"lib{base_name}.dll"]
    elif sys.platform == "darwin":
        names = [f"lib{base_name}.dylib", f"{base_name}.dylib"]
    else:
        names = [f"lib{base_name}.so", f"{base_name}.so"]

    search_dirs = [
        os.path.join(root_dir, "target", "debug"),
        os.path.join(root_dir, "target", "release"),
        os.path.join(root_dir, "target", "x86_64-pc-windows-msvc", "debug"),
        os.path.join(root_dir, "target", "x86_64-unknown-linux-gnu", "debug"),
        os.path.join(root_dir, "target", "aarch64-apple-darwin", "debug"),
    ]
    for directory in search_dirs:
        for name in names:
            candidate = os.path.join(directory, name)
            if os.path.isfile(candidate):
                return candidate
    return os.path.join(root_dir, "target", "debug", names[0])


def run_checked(command, cwd, env, label):
    print(f"[test] $ {' '.join(command)}", flush=True)
    result = subprocess.run(command, cwd=cwd, env=env, text=True, capture_output=True)
    if result.stdout:
        print(result.stdout, end="", flush=True)
    if result.stderr:
        print(result.stderr, end="", file=sys.stderr, flush=True)
    if result.returncode != 0:
        raise AssertionError(f"{label} failed with exit code {result.returncode}")
    return result


def read_until(process, markers, timeout_seconds, output_lines):
    deadline = time.time() + timeout_seconds
    while time.time() < deadline:
        line = process.stdout.readline()
        if not line:
            return None
        output_lines.append(line)
        print(line, end="", flush=True)
        if any(marker in line for marker in markers):
            return line
    return None


def drain_output(process, output_lines):
    while True:
        line = process.stdout.readline()
        if not line:
            break
        output_lines.append(line)
        print(line, end="", flush=True)


def main():
    root_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

    cli = find_cli_binary(root_dir)
    if not os.path.isfile(cli):
        print(f"[test] ERROR: cargo-redox-mc binary not found: {cli}", file=sys.stderr)
        return 1

    agent_lib = os.environ.get("AGENT_LIB") or find_library("redox_mc_loader", root_dir)
    if not os.path.isfile(agent_lib):
        print(f"[test] ERROR: loader agent not found: {agent_lib}", file=sys.stderr)
        return 1

    server_jar = os.environ.get("BUNDLER_JAR") or os.environ.get("MINECRAFT_BUNDLER_JAR")
    if not server_jar or not os.path.isfile(server_jar):
        for candidate in (
            os.path.join(root_dir, "versions", "26.3", "server.jar"),
            os.path.expanduser("~/server.jar"),
        ):
            if os.path.isfile(candidate):
                server_jar = candidate
                break
    if not server_jar or not os.path.isfile(server_jar):
        print(f"[test] ERROR: server jar not found: {server_jar}", file=sys.stderr)
        return 1

    print(f"[test] CLI binary:  {cli}")
    print(f"[test] Loader agent: {agent_lib}")
    print(f"[test] Server jar:   {server_jar}")

    work_dir = tempfile.mkdtemp(prefix="redox_cli_e2e_")
    project_dir = os.path.join(work_dir, "demo_mod")
    port = get_free_port()

    env = dict(os.environ)
    env["REDOX_MC_HOME"] = to_native_path(root_dir)
    env["REDOX_MC_AGENT_LIB"] = to_native_path(agent_lib)
    env["REDOX_MC_SERVER_JAR"] = to_native_path(server_jar)
    # Reuse the workspace build cache so scaffolding compiles quickly.
    env["CARGO_TARGET_DIR"] = to_native_path(os.path.join(root_dir, "target"))

    process = None
    output_lines = []
    try:
        print("[test] Scaffolding a mod with `cargo redox-mc new`...")
        run_checked(
            [cli, "redox-mc", "new", "demo_mod", "--dir", to_native_path(work_dir), "--author", "CI"],
            cwd=root_dir,
            env=env,
            label="cargo redox-mc new",
        )

        for relative in ("Cargo.toml", "mod.toml", "src/lib.rs", ".gitignore"):
            path = os.path.join(project_dir, relative)
            assert os.path.isfile(path), f"scaffold is missing {relative}"

        mod_toml = open(os.path.join(project_dir, "mod.toml"), encoding="utf-8").read()
        assert 'id = "demo_mod"' in mod_toml
        assert 'side = "both"' in mod_toml

        cargo_toml = open(os.path.join(project_dir, "Cargo.toml"), encoding="utf-8").read()
        assert 'crate-type = ["cdylib"]' in cargo_toml

        print("[test] Building the scaffolded mod with `cargo redox-mc build`...")
        run_checked(
            [cli, "redox-mc", "build"],
            cwd=project_dir,
            env=env,
            label="cargo redox-mc build",
        )
        dist_dir = os.path.join(project_dir, ".redox-mc", "dist", "demo_mod")
        assert os.path.isfile(os.path.join(dist_dir, mod_library_name())), (
            f"expected a staged cdylib in {dist_dir}"
        )
        assert os.path.isfile(os.path.join(dist_dir, "mod.toml"))

        print("[test] Launching the environment with `cargo redox-mc run --server`...")
        process = subprocess.Popen(
            [cli, "redox-mc", "run", "--server", "--port", str(port)],
            cwd=project_dir,
            env=env,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
        )

        ready = read_until(process, ["Done ("], 180, output_lines)
        assert ready is not None, "server did not reach the ready state"

        startup = "".join(output_lines)
        assert "[demo_mod] initialized" in startup, "the scaffolded mod must run redox_mod_init"
        assert "Loading native mod 'demo_mod'" in startup, "the loader must load demo_mod"
        assert "[demo_mod] bootstrap hook" in startup, "the scaffolded mod must get the bootstrap hook"

        server_mod_dir = os.path.join(project_dir, ".redox-mc", "server", "mods", "demo_mod")
        assert os.path.isdir(server_mod_dir), "run must stage the mod for the server"
        assert os.path.isfile(os.path.join(server_mod_dir, "mod.toml"))

        # Hot-reload loop: change the source, rebuild in a separate process and
        # ask the running server to reload the new generation.
        print("[test] Editing the mod source and rebuilding for a live /redox reload...")
        lib_rs = os.path.join(project_dir, "src", "lib.rs")
        source = open(lib_rs, encoding="utf-8").read()
        source = source.replace('"[demo_mod] initialized"', '"[demo_mod] v2 initialized"')
        open(lib_rs, "w", encoding="utf-8").write(source)

        run_checked(
            [cli, "redox-mc", "build"],
            cwd=project_dir,
            env=env,
            label="cargo redox-mc build (hot reload)",
        )

        process.stdin.write("/redox reload\n")
        process.stdin.flush()
        reload_line = read_until(
            process,
            ["Reloaded 1 native mod(s)", "Redox reload failed"],
            60,
            output_lines,
        )
        assert reload_line is not None, "the server did not report the reload result"
        assert "Redox reload failed" not in reload_line, reload_line
        assert "[demo_mod] v2 initialized" in "".join(output_lines), (
            "the reloaded generation must execute the new code"
        )

        process.stdin.write("say cli-reload-ok\n")
        process.stdin.flush()
        assert read_until(process, ["cli-reload-ok"], 20, output_lines) is not None, (
            "the server must stay responsive after the reload"
        )

        process.stdin.write("stop\n")
        process.stdin.flush()
        drain_output(process, output_lines)
        process.wait(timeout=30)

        full_output = "".join(output_lines)
        assert "VerifyError" not in full_output, "the workflow must not produce VerifyError"
        assert re.search(r"Unknown or incomplete command.*redox", full_output) is None, (
            "/redox reload must be handled by the loader"
        )
        assert process.returncode == 0, f"server exited with {process.returncode}"

        print("\n>>> TASK 7.2 CARGO-REDOX-MC END-TO-END TEST PASSED! <<<")
        return 0

    finally:
        if process is not None and process.poll() is None:
            process.kill()
        for _ in range(5):
            try:
                shutil.rmtree(work_dir, ignore_errors=False)
                break
            except OSError:
                time.sleep(0.2)


if __name__ == "__main__":
    sys.exit(main())