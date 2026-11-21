#!/usr/bin/env python3
"""Integration test for Task 8.1: Headless Minecraft server BlockEntity validation across Linux, macOS, and Windows.

Validates:
1. BlockEntityType registration and EntityBlock wiring.
2. Server-side BlockEntity ticking at 20 Hz.
3. Persistent state serialization via MC 26.3 ValueOutput on server shutdown.
4. Persistent state deserialization via MC 26.3 ValueInput on server restart.
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


def run_server_session(cmd, test_dir, commands, timeout_seconds=90):
    print(f"[test] Launching headless server in {test_dir}...")
    proc = subprocess.Popen(
        cmd,
        cwd=test_dir,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1
    )

    output_lines = []
    ready = False
    start_time = time.time()

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
        return False, "".join(output_lines)

    print("=== SERVER READY, SENDING COMMANDS ===", flush=True)
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
    return True, "".join(output_lines)


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

    test_dir = tempfile.mkdtemp(prefix="redox_task81_")
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
        shutil.copy(mod_lib, mods_dir)

        native_agent = to_native_path(agent_lib)
        native_jar = to_native_path(server_jar)

        cmd = [
            "java",
            f"-agentpath:{native_agent}",
            "-jar", native_jar,
            "nogui"
        ]

        print("\n=== PHASE 1: PLACE BLOCK ENTITY, TICK, AND SAVE ===")
        session1_commands = [
            ("forceload add 0 0\n", 1.0),
            ("setblock 0 64 0 testmod:counter_block\n", 3.0),
            ("save-all\n", 1.0),
            ("stop\n", 0.0),
        ]
        ok, out1 = run_server_session(cmd, test_dir, session1_commands)
        if not ok:
            return 1

        assert "Registered custom entity block into BuiltInRegistries.BLOCK: 'testmod:counter_block'" in out1, \
            "Expected testmod:counter_block to be registered into BuiltInRegistries.BLOCK"
        assert "Changed the block at 0, 64, 0" in out1, \
            "Expected setblock 0 64 0 testmod:counter_block to succeed"
        assert "[test-mod] BlockEntity tick at (0, 64, 0)" in out1, \
            "Expected testmod:counter_be to tick at (0, 64, 0)"
        assert "[test-mod] BlockEntity save at (0, 64, 0)" in out1, \
            "Expected testmod:counter_be to save state on world save"

        print("\n=== PHASE 2: RESTART SERVER AND VERIFY RESTORED STATE ===")
        session2_commands = [
            ("forceload add 0 0\n", 2.0),
            ("stop\n", 0.0),
        ]
        ok, out2 = run_server_session(cmd, test_dir, session2_commands)
        if not ok:
            return 1

        assert "[test-mod] BlockEntity load at (0, 64, 0)" in out2, \
            "Expected testmod:counter_be to load state on world load"
        match = re.search(r"\[test-mod\] BlockEntity restored counter=(\d+)", out2)
        assert match is not None, "Expected testmod:counter_be to restore counter from saved state"
        restored_count = int(match.group(1))
        assert restored_count > 0, f"Expected restored counter > 0, got {restored_count}"
        print(f"Verified restored counter value across server restart: {restored_count}")

        print("\n>>> TASK 8.1 BLOCK ENTITY HEADLESS VALIDATION PASSED! <<<")
        return 0

    finally:
        for _ in range(5):
            try:
                shutil.rmtree(test_dir, ignore_errors=False)
                break
            except Exception:
                time.sleep(0.5)
        shutil.rmtree(test_dir, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
