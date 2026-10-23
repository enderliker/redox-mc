#!/usr/bin/env python3
"""Integration test for Task 7.1: safe native hot reload via `/redox reload`.

The test starts a real headless Minecraft server with the first-generation
fixture mod, overwrites the project binary while the server is running, and
asks the loader to reload. A successful run proves that:

* the original binary is not locked by the JVM (the overwrite succeeds),
* `/redox reload` is intercepted before Brigadier rejects it,
* the new generation is initialized and published without a JVM restart,
* the server remains responsive and does not crash after the swap.
"""

import os
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
        os.path.join(root_dir, "target", "x86_64-pc-windows-gnu", "debug"),
        os.path.join(root_dir, "target", "x86_64-unknown-linux-gnu", "debug"),
        os.path.join(root_dir, "target", "aarch64-apple-darwin", "debug"),
        os.path.join(root_dir, "target", "x86_64-apple-darwin", "debug"),
    ]

    for directory in search_dirs:
        for name in names:
            candidate = os.path.join(directory, name)
            if os.path.isfile(candidate):
                return candidate

    return os.path.join(root_dir, "target", "debug", names[0])


def mod_binary_name():
    if sys.platform == "win32" or os.name == "nt":
        return "test_mod.dll"
    if sys.platform == "darwin":
        return "libtest_mod.dylib"
    return "libtest_mod.so"


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

    agent_lib = os.environ.get("AGENT_LIB") or find_library("redox_mc_loader", root_dir)
    v1_lib = os.environ.get("RELOAD_MOD_V1_LIB") or find_library("reload_mod_v1", root_dir)
    v2_lib = os.environ.get("RELOAD_MOD_V2_LIB") or find_library("reload_mod_v2", root_dir)
    server_jar = os.environ.get("BUNDLER_JAR") or os.environ.get("MINECRAFT_BUNDLER_JAR")

    if not server_jar or not os.path.exists(server_jar):
        for candidate in (
            os.path.join(root_dir, "versions", "26.3", "server.jar"),
            os.path.expanduser("~/server.jar"),
        ):
            if os.path.isfile(candidate):
                server_jar = candidate
                break

    for label, path in (
        ("Agent library", agent_lib),
        ("Hot-reload v1 fixture", v1_lib),
        ("Hot-reload v2 fixture", v2_lib),
        ("Bundler jar", server_jar),
    ):
        if not path or not os.path.exists(path):
            print(f"{label} not found: {path}", file=sys.stderr)
            return 1

    print(f"[test] Agent library: {agent_lib}")
    print(f"[test] v1 fixture:    {v1_lib}")
    print(f"[test] v2 fixture:    {v2_lib}")
    print(f"[test] Bundler jar:   {server_jar}")

    test_dir = tempfile.mkdtemp(prefix="redox_hot_reload_")
    port = get_free_port()
    output_lines = []
    process = None

    try:
        with open(os.path.join(test_dir, "eula.txt"), "w", encoding="utf-8") as handle:
            handle.write("eula=true\n")
        with open(os.path.join(test_dir, "server.properties"), "w", encoding="utf-8") as handle:
            handle.write(
                "online-mode=false\n"
                f"server-port={port}\n"
                "sync-chunk-writes=false\n"
                "level-type=minecraft:flat\n"
                "gamemode=survival\n"
                "difficulty=peaceful\n"
                "spawn-protection=0\n"
            )

        mods_dir = os.path.join(test_dir, "mods", "test_mod")
        os.makedirs(mods_dir, exist_ok=True)
        shutil.copy(
            os.path.join(root_dir, "examples", "test-mod", "mod.toml"),
            os.path.join(mods_dir, "mod.toml"),
        )
        project_binary = os.path.join(mods_dir, mod_binary_name())
        shutil.copy(v1_lib, project_binary)

        command = [
            "java",
            f"-agentpath:{to_native_path(agent_lib)}",
            "-jar",
            to_native_path(server_jar),
            "nogui",
        ]
        print(f"[test] Launching headless server in {test_dir} (port={port})...")
        process = subprocess.Popen(
            command,
            cwd=test_dir,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
        )

        ready = read_until(process, ["Done ("], 90, output_lines)
        if not ready:
            print("[test] ERROR: server did not reach ready state", file=sys.stderr)
            return 1

        startup_output = "".join(output_lines)
        assert "[reload-fixture-v1] initialized" in startup_output, (
            "the v1 fixture must be loaded during startup"
        )
        cache_dir = os.path.join(test_dir, "mod_cache")
        assert os.path.isdir(cache_dir), "the loader must create mod_cache/"
        assert any(name.startswith("mod_") for name in os.listdir(cache_dir)), (
            "the initial load must use a cached copy"
        )

        # Replacing the project binary while the JVM runs is the Windows
        # no-lock acceptance check. It also prepares the v2 generation.
        print("[test] Replacing the project mod binary with the v2 generation...")
        shutil.copy(v2_lib, project_binary)

        print("[test] Sending /redox reload...")
        process.stdin.write("/redox reload\n")
        process.stdin.flush()

        reload_result = read_until(
            process,
            ["Reloaded 1 native mod(s)", "Redox reload failed"],
            30,
            output_lines,
        )
        assert reload_result is not None, "the server did not report the reload result"
        assert "Redox reload failed" not in reload_result, reload_result

        reload_output = "".join(output_lines)
        assert "[reload-fixture-v2] initialized" in reload_output, (
            "the v2 fixture must be initialized by the reload"
        )
        assert "[reload-fixture-v2] bootstrap hook" in reload_output, (
            "the new generation must receive the bootstrap hook"
        )
        assert "Unknown or incomplete command" not in reload_output, (
            "/redox reload must be intercepted before Brigadier rejects it"
        )

        # The server must remain responsive after the callback swap.
        process.stdin.write("say redox-reload-ok\n")
        process.stdin.flush()
        assert read_until(process, ["redox-reload-ok"], 15, output_lines) is not None, (
            "the server must still process commands after the reload"
        )

        process.stdin.write("stop\n")
        process.stdin.flush()
        drain_output(process, output_lines)
        process.wait(timeout=30)

        full_output = "".join(output_lines)
        assert "VerifyError" not in full_output, "hot reload must not produce VerifyError"
        assert "SIGSEGV" not in full_output, "hot reload must not crash the JVM"
        assert process.returncode == 0, f"server exited with {process.returncode}"

        cache_files = [name for name in os.listdir(cache_dir) if name.startswith("mod_")]
        assert len(cache_files) >= 2, (
            f"both load generations must have cached copies, found {cache_files}"
        )

        print("\n>>> TASK 7.1 HOT-RELOAD INTEGRATION TEST PASSED! <<<")
        return 0

    finally:
        if process is not None and process.poll() is None:
            process.kill()
        for _ in range(5):
            try:
                shutil.rmtree(test_dir, ignore_errors=False)
                break
            except OSError:
                time.sleep(0.2)


if __name__ == "__main__":
    sys.exit(main())
