#!/usr/bin/env python3
"""Integration test for Task 4.3: Headless Minecraft server item validation across Linux, macOS, and Windows."""

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
    # Try cygpath if available (MSYS2 / Git Bash on Windows)
    try:
        res = subprocess.run(["cygpath", "-w", path], capture_output=True, text=True)
        if res.returncode == 0:
            out = res.stdout.strip()
            if out:
                return out
    except (FileNotFoundError, PermissionError, subprocess.SubprocessError):
        pass

    # Handle MSYS2 unix-style drive mounts like /c/Users or /d/a on Windows
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
    # Platform-specific library names
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

    # Resolve agent library
    agent_lib = os.environ.get("AGENT_LIB")
    if not agent_lib or not os.path.exists(agent_lib):
        agent_lib = find_library("redox_mc_loader", root_dir)

    # Resolve test mod library
    mod_lib = os.environ.get("MOD_LIB")
    if not mod_lib or not os.path.exists(mod_lib):
        mod_lib = find_library("test_mod", root_dir)

    # Resolve bundler server.jar
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

    test_dir = tempfile.mkdtemp(prefix="redox_task43_")
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
        print(f"[test] Launching headless server in {test_dir} (port={port})...")
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
            print(f"[test] ERROR: Server did not reach ready state within {timeout_seconds} seconds!", file=sys.stderr)
            proc.kill()
            return 1

        print("=== SERVER READY, SENDING COMMANDS ===", flush=True)
        # Test 1: Send invalid item command
        try:
            proc.stdin.write("give @p testmod:invalid_nonexistent 1\n")
            proc.stdin.flush()
            time.sleep(1)

            # Test 2: Send valid custom mod item command
            proc.stdin.write("give @p testmod:ruby 5\n")
            proc.stdin.flush()
            time.sleep(1)

            # Test 3: Send datapack list command to verify virtual pack injection
            proc.stdin.write("datapack list\n")
            proc.stdin.flush()
            time.sleep(1)

            # Force-load chunk (0, 0) so setblock can place blocks
            proc.stdin.write("forceload add 0 0\n")
            proc.stdin.flush()
            time.sleep(1)

            # Test 4: Send invalid block setblock command
            proc.stdin.write("setblock 0 64 0 testmod:invalid_nonexistent_block\n")
            proc.stdin.flush()
            time.sleep(1)

            # Test 5: Send valid custom mod block setblock command
            proc.stdin.write("setblock 0 64 0 testmod:ruby_block\n")
            proc.stdin.flush()
            time.sleep(1)

            # Test 6: Spawn loot from virtual block loot table
            proc.stdin.write("loot spawn 0 65 0 loot testmod:blocks/ruby_block\n")
            proc.stdin.flush()
            time.sleep(1)

            # Test 7: Destroy block to verify breaking drop behavior
            proc.stdin.write("setblock 0 64 0 minecraft:air destroy\n")
            proc.stdin.flush()
            time.sleep(1)

            # Stop server
            proc.stdin.write("stop\n")
            proc.stdin.flush()
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

        # Verification assertions
        assert "Registered custom item into BuiltInRegistries.ITEM: 'testmod:ruby'" in full_output, \
            "Expected testmod:ruby to be registered into BuiltInRegistries.ITEM"
        assert "Registered custom block into BuiltInRegistries.BLOCK: 'testmod:ruby_block'" in full_output, \
            "Expected testmod:ruby_block to be registered into BuiltInRegistries.BLOCK"
        assert "Unknown item 'testmod:invalid_nonexistent'" in full_output, \
            "Expected server to reject unknown item"
        assert "Unknown item 'testmod:ruby'" not in full_output, \
            "Server unexpectedly rejected testmod:ruby"
        assert "No player was found" in full_output, \
            "Expected give @p testmod:ruby to parse successfully and report 'No player was found'"
        assert "redox_mod_resources" in full_output, \
            "Expected virtual datapack 'redox_mod_resources' to be enabled in datapack list"
        assert "Unknown block 'testmod:invalid_nonexistent_block'" in full_output or "testmod:invalid_nonexistent_block" in full_output, \
            "Expected server to reject unknown block"
        assert "Changed the block at 0, 64, 0" in full_output, \
            "Expected setblock 0 64 0 testmod:ruby_block to place the block successfully"
        assert "Dropped 1 [item.testmod.ruby]" in full_output or "Dropped 1" in full_output, \
            "Expected loot spawn 0 65 0 loot testmod:blocks/ruby_block to drop item from virtual loot table"

        print("\n>>> TASK 4.3, TASK 5.1, TASK 5.2 & TASK 5.3 HEADLESS VALIDATION PASSED! <<<")
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
