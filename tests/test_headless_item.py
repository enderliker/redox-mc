#!/usr/bin/env python3
"""Integration test for Task 4.3: Headless Minecraft server item validation."""

import os
import shutil
import subprocess
import sys
import tempfile
import time

def main():
    root_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    agent_lib = os.path.join(root_dir, "target", "debug", "libredox_mc_loader.so")
    mod_lib = os.path.join(root_dir, "target", "debug", "libtest_mod.so")
    server_jar = os.path.join(root_dir, "versions", "26.3", "server.jar")

    if not os.path.exists(agent_lib):
        print(f"Agent library not found at: {agent_lib}", file=sys.stderr)
        return 1
    if not os.path.exists(mod_lib):
        print(f"Mod library not found at: {mod_lib}", file=sys.stderr)
        return 1
    if not os.path.exists(server_jar):
        print(f"Server jar not found at: {server_jar}", file=sys.stderr)
        return 1

    test_dir = tempfile.mkdtemp(prefix="redox_task43_")
    try:
        with open(os.path.join(test_dir, "eula.txt"), "w") as f:
            f.write("eula=true\n")
        with open(os.path.join(test_dir, "server.properties"), "w") as f:
            f.write(
                "online-mode=false\n"
                "server-port=25596\n"
                "sync-chunk-writes=false\n"
                "level-type=minecraft:flat\n"
                "gamemode=survival\n"
            )

        mods_dir = os.path.join(test_dir, "mods", "test_mod")
        os.makedirs(mods_dir, exist_ok=True)
        shutil.copy(os.path.join(root_dir, "examples", "test-mod", "mod.toml"), mods_dir)
        shutil.copy(mod_lib, mods_dir)

        cmd = [
            "java",
            f"-agentpath:{agent_lib}",
            "-jar", server_jar,
            "nogui"
        ]
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

        while time.time() - start_time < 45:
            line = proc.stdout.readline()
            if not line:
                break
            output_lines.append(line)
            print(line, end="", flush=True)
            if "Done (" in line:
                ready = True
                break

        if not ready:
            print("[test] ERROR: Server did not reach ready state within 45 seconds!", file=sys.stderr)
            proc.kill()
            return 1

        print("=== SERVER READY, SENDING COMMANDS ===", flush=True)
        # Test 1: Send invalid item command
        proc.stdin.write("give @p testmod:invalid_nonexistent 1\n")
        proc.stdin.flush()
        time.sleep(1)

        # Test 2: Send valid custom mod item command
        proc.stdin.write("give @p testmod:ruby 5\n")
        proc.stdin.flush()
        time.sleep(1)

        # Stop server
        proc.stdin.write("stop\n")
        proc.stdin.flush()

        while True:
            line = proc.stdout.readline()
            if not line:
                break
            output_lines.append(line)
            print(line, end="", flush=True)

        proc.wait(timeout=15)

        full_output = "".join(output_lines)

        # Verification assertions
        assert "Registered custom item into BuiltInRegistries.ITEM: 'testmod:ruby'" in full_output, \
            "Expected testmod:ruby to be registered into BuiltInRegistries.ITEM"
        assert "Unknown item 'testmod:invalid_nonexistent'" in full_output, \
            "Expected server to reject unknown item"
        assert "Unknown item 'testmod:ruby'" not in full_output, \
            "Server unexpectedly rejected testmod:ruby"
        assert "No player was found" in full_output, \
            "Expected give @p testmod:ruby to parse successfully and report 'No player was found'"

        print("\n>>> TASK 4.3 HEADLESS ITEM VALIDATION PASSED! <<<")
        return 0

    finally:
        shutil.rmtree(test_dir, ignore_errors=True)

if __name__ == "__main__":
    sys.exit(main())
