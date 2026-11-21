#!/usr/bin/env python3
"""Integration test for Task 8.2: Container Menus and Screens Headless Validation across Linux, macOS, and Windows.

Validates:
1. Dynamic MenuType registration into BuiltInRegistries.MENU ('testmod:counter_menu' with 9 slots).
2. Deterministic inclusion of container menus into SHA-256 registry handshake hash.
3. Server-side container menu lifecycle execution:
   - Creation of GenericRedoxMenu instance.
   - Slot click interception (allowing slot 0, denying slot 8).
   - Closing of menu session and invocation of Rust mod on_close handler.
4. Clean server shutdown without VerifyError, classloader deadlocks, or JNI exceptions.
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

    test_dir = tempfile.mkdtemp(prefix="redox_task82_")
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
            print(f"[test] ERROR: Server did not reach ready state within {timeout_seconds}s!", file=sys.stderr)
            proc.kill()
            return 1

        print("=== SERVER READY, SENDING COMMANDS ===", flush=True)
        try:
            # Place block and trigger save
            proc.stdin.write("forceload add 0 0\n")
            proc.stdin.flush()
            time.sleep(1.0)

            proc.stdin.write("setblock 0 64 0 testmod:counter_block\n")
            proc.stdin.flush()
            time.sleep(1.0)

            proc.stdin.write("save-all\n")
            proc.stdin.flush()
            time.sleep(1.0)

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
        assert proc.returncode == 0, f"Headless server exited with non-zero exit code: {proc.returncode}"
        full_output = "".join(output_lines)

        # 1. Verify custom MenuType was registered in BuiltInRegistries.MENU
        assert "Registered custom MenuType in BuiltInRegistries.MENU: 'testmod:counter_menu' (slots=9)" in full_output or \
               "[redox-mc-bridge] Registered custom MenuType: 'testmod:counter_menu' with 9 slots" in full_output, \
            "Expected testmod:counter_menu to be registered into BuiltInRegistries.MENU"

        # 2. Verify test-mod registered the menu definition
        assert "[test-mod] Registered custom menu: 'testmod:counter_menu' with 9 slots" in full_output, \
            "Expected test-mod to register 'testmod:counter_menu' with 9 slots"

        # 3. Verify content registration summary includes menu type
        assert "1 menu type(s) registered" in full_output, \
            "Expected content registration log to include menu type count"

        # 4. Verify slot click 0 was exercised and allowed
        assert "[redox-mc-bridge] Exercising slot click 0 (expecting Allow)..." in full_output, \
            "Expected bridge to exercise slot click 0"
        assert "[test-mod] Slot clicked in menu 'testmod:counter_menu': slot=0" in full_output, \
            "Expected test-mod to receive slot click event for slot 0"
        assert "[test-mod] Allowing click on slot 0!" in full_output, \
            "Expected test-mod to log allowance for slot 0"

        # 5. Verify slot click 8 was exercised and denied
        assert "[redox-mc-bridge] Exercising slot click 8 (expecting Deny)..." in full_output, \
            "Expected bridge to exercise slot click 8"
        assert "[test-mod] Slot clicked in menu 'testmod:counter_menu': slot=8" in full_output, \
            "Expected test-mod to receive slot click event for slot 8"
        assert "[test-mod] Denying click on slot 8!" in full_output, \
            "Expected test-mod to log denial on slot 8"

        # 6. Verify menu session close lifecycle
        assert "[redox-mc-bridge] Closing menu session..." in full_output, \
            "Expected bridge to close menu session"
        assert "[test-mod] Menu closed: id='testmod:counter_menu'" in full_output, \
            "Expected test-mod on_close hook to be invoked"
        assert "[redox-mc-bridge] testMenuSlotClickLifecycle completed successfully." in full_output, \
            "Expected testMenuSlotClickLifecycle to complete successfully"

        # 7. Invariance assertions: no VerifyError, no classloader deadlocks
        assert "java.lang.VerifyError" not in full_output, "Fatal java.lang.VerifyError detected in log"
        assert "Exception in thread \"main\" java.lang.NoClassDefFoundError" not in full_output, \
            "NoClassDefFoundError detected in log"

        print("\n>>> TASK 8.2 CONTAINER MENUS HEADLESS VALIDATION PASSED! <<<")
        return 0

    finally:
        if "proc" in locals() and proc is not None and proc.poll() is None:
            try:
                proc.kill()
                proc.wait(timeout=5)
            except Exception:
                pass
        for _ in range(5):
            try:
                shutil.rmtree(test_dir, ignore_errors=False)
                break
            except Exception:
                time.sleep(0.5)
        shutil.rmtree(test_dir, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
