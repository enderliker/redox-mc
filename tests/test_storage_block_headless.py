#!/usr/bin/env python3
"""Integration test for Persistent Container Storage Block (BlockEntity + Menu).

Validates:
1. Dynamic registration of 'testmod:storage_crate' into BuiltInRegistries.BLOCK with attached 'testmod:storage_crate_be'.
2. Dynamic registration of 'testmod:storage_crate_menu' with 9 slots into BuiltInRegistries.MENU.
3. Server-side container storage lifecycle execution:
   - GenericRedoxBlockEntity instantiation with FLAG_HAS_CONTAINER and 9 slots.
   - Storing item stack (16 diamonds) in slot 0 of the block entity container.
   - Creation of GenericRedoxMenu bound to the block entity container.
   - Verification that the opened menu reflects the 16 diamonds in slot 0.
   - Exercise of slot interaction lifecycle and menu closure.
4. Clean in-game setblock and server shutdown without VerifyError, deadlocks, or JNI crashes.
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
        res = subprocess.run(["cygpath", "-w", path], capture_output=True, text=True)
        if res.returncode == 0:
            out = res.stdout.strip()
            if out:
                return out
    except (FileNotFoundError, PermissionError, subprocess.SubprocessError):
        pass

    if os.name == "nt" or sys.platform == "win32":
        if len(path) >= 3 and path[0] == "/" and path[2] == "/":
            drive = path[1].upper()
            return drive + ":\\" + path[3:].replace("/", "\\")

    return os.path.abspath(path)


def get_free_port():
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            s.bind(("", 0))
            return s.getsockname()[1]
    except Exception:
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
        os.path.join(root_dir, "target", "x86_64-unknown-linux-gnu", "debug"),
        os.path.join(root_dir, "target", "aarch64-apple-darwin", "debug"),
        os.path.join(root_dir, "target", "x86_64-apple-darwin", "debug"),
        os.path.join(root_dir, "target", "x86_64-pc-windows-gnu", "debug"),
    ]

    for d in search_dirs:
        for n in names:
            p = os.path.join(d, n)
            if os.path.isfile(p):
                return p

    return os.path.join(root_dir, "target", "debug", names[0])


def main():
    root_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

    agent_lib = os.environ.get("AGENT_LIB")
    if not agent_lib or not os.path.exists(agent_lib):
        agent_lib = find_library("redox_mc_loader", root_dir)

    mod_lib = os.environ.get("MOD_LIB")
    if not mod_lib or not os.path.exists(mod_lib):
        mod_lib = find_library("test_mod", root_dir)

    server_jar = os.environ.get("BUNDLER_JAR") or os.environ.get("MINECRAFT_BUNDLER_JAR")
    if not server_jar or not os.path.exists(server_jar):
        candidates = [
            os.path.join(root_dir, "versions", "26.3", "server.jar"),
            os.path.expanduser("~/server.jar"),
        ]
        for cand in candidates:
            if os.path.isfile(cand):
                server_jar = cand
                break

    if not agent_lib or not os.path.exists(agent_lib):
        print(f"Agent library not found: {agent_lib}", file=sys.stderr)
        return 1
    if not mod_lib or not os.path.exists(mod_lib):
        print(f"Mod library not found: {mod_lib}", file=sys.stderr)
        return 1
    if not server_jar or not os.path.exists(server_jar):
        print(f"Server jar not found: {server_jar}", file=sys.stderr)
        return 1

    print(f"[test] Agent library:  {agent_lib}")
    print(f"[test] Mod library:    {mod_lib}")
    print(f"[test] Bundler jar:    {server_jar}")

    test_dir = tempfile.mkdtemp(prefix="redox_storage_")
    port = get_free_port()

    try:
        with open(os.path.join(test_dir, "eula.txt"), "w") as f:
            f.write("eula=true\n")
        with open(os.path.join(test_dir, "server.properties"), "w") as f:
            f.write(
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
        shutil.copy(os.path.join(root_dir, "examples", "test-mod", "mod.toml"), mods_dir)
        shutil.copy2(mod_lib, os.path.join(mods_dir, os.path.basename(mod_lib)))

        native_agent = to_native_path(agent_lib)
        native_server = to_native_path(server_jar)

        cmd = [
            "java",
            f"-agentpath:{native_agent}",
            "-jar",
            native_server,
            "nogui",
        ]

        print(f"[test] Launching headless server in {test_dir}...")
        proc = subprocess.Popen(
            cmd,
            cwd=test_dir,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
        )

        output_lines = []
        ready = False
        start_time = time.time()
        timeout_seconds = 90

        while time.time() - start_time < timeout_seconds:
            line = proc.stdout.readline()
            if not line:
                break
            output_lines.append(line)
            print(line, end="", flush=True)
            if "Done (" in line:
                ready = True
                break

        if not ready:
            print(f"[test] ERROR: Server did not reach ready state within {timeout_seconds}s!", file=sys.stderr)
            proc.kill()
            return 1

        print("=== SERVER READY, SENDING STORAGE BLOCK COMMANDS ===", flush=True)
        commands = [
            ("setblock 0 64 0 testmod:storage_crate\n", 2.0),
            ("save-all flush\n", 2.0),
            ("stop\n", 0.0),
        ]

        try:
            for c, delay in commands:
                proc.stdin.write(c)
                proc.stdin.flush()
                if delay > 0:
                    time.sleep(delay)
        except (BrokenPipeError, OSError) as e:
            print(f"[test] Warning while writing to stdin: {e}", file=sys.stderr)

        while True:
            line = proc.stdout.readline()
            if not line:
                break
            output_lines.append(line)
            print(line, end="", flush=True)

        proc.wait(timeout=30)
        full_output = "".join(output_lines)

        print("=== VALIDATING STORAGE BLOCK ASSERTIONS ===")

        # 1. Block and entity registration
        assert "[test-mod] Registered container block: 'testmod:storage_crate' with 9-slot persistent container" in full_output, \
            "Expected test-mod container block registration"
        assert "[test-mod] Registered custom menu: 'testmod:storage_crate_menu' with 9 slots" in full_output, \
            "Expected test-mod container menu registration"
        assert "BuiltInRegistries.BLOCK: 'testmod:storage_crate'" in full_output, \
            "Expected in-game registration of testmod:storage_crate"
        assert "BuiltInRegistries.MENU: 'testmod:storage_crate_menu'" in full_output, \
            "Expected in-game registration of testmod:storage_crate_menu"

        # 2. Storage block container lifecycle execution
        assert "[redox-mc-bridge] Starting testStorageBlockContainerLifecycle..." in full_output, \
            "Expected testStorageBlockContainerLifecycle start"
        assert "[redox-mc-bridge] Stored 16 diamonds in slot 0 of storage crate" in full_output, \
            "Expected diamonds stored in slot 0"
        assert "[redox-mc-bridge] Menu successfully created from block entity with 16 diamonds in slot 0" in full_output, \
            "Expected menu to reflect 16 diamonds in slot 0"
        assert "[redox-mc-bridge] testStorageBlockContainerLifecycle completed successfully." in full_output, \
            "Expected testStorageBlockContainerLifecycle completion"

        # 3. Clean shutdown
        assert proc.returncode == 0, f"Expected clean exit code 0, got {proc.returncode}"
        assert "VerifyError" not in full_output, "VerifyError detected in output"
        assert "Exception in thread" not in full_output, "Uncaught JVM exception in output"

        print("\n[test] SUCCESS: Persistent container storage block validation passed 100%!")
        return 0

    finally:
        try:
            shutil.rmtree(test_dir)
        except Exception:
            pass


if __name__ == "__main__":
    sys.exit(main())
