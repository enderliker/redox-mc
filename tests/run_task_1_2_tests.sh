#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

case "$(uname -s)" in
    CYGWIN*|MINGW*|MSYS*)
        AGENT_LIB_BASENAME="redox_mc_loader.dll"
        MOD_LIB_BASENAME="test_mod.dll"
        CP_SEP=";"
        ;;
    Darwin*)
        AGENT_LIB_BASENAME="libredox_mc_loader.dylib"
        MOD_LIB_BASENAME="libtest_mod.dylib"
        CP_SEP=":"
        ;;
    *)
        AGENT_LIB_BASENAME="libredox_mc_loader.so"
        MOD_LIB_BASENAME="libtest_mod.so"
        CP_SEP=":"
        ;;
esac

find_library() {
    local base_name="$1"
    for cand in \
        "${ROOT_DIR}/target/debug/${base_name}" \
        "${ROOT_DIR}/target/debug/lib${base_name}" \
        "${ROOT_DIR}/target/x86_64-pc-windows-msvc/debug/${base_name}" \
        "${ROOT_DIR}/target/x86_64-pc-windows-msvc/debug/lib${base_name}" \
        "${ROOT_DIR}/target/x86_64-unknown-linux-gnu/debug/${base_name}" \
        "${ROOT_DIR}/target/aarch64-apple-darwin/debug/${base_name}"; do
        if [ -f "${cand}" ]; then
            echo "${cand}"
            return 0
        fi
    done
    echo "${ROOT_DIR}/target/debug/${base_name}"
}

to_native_path() {
    if command -v cygpath >/dev/null 2>&1; then
        cygpath -w "$1"
    else
        echo "$1"
    fi
}

echo "=== Building redox-mc-bridge and compiling agent ==="
"${ROOT_DIR}/redox-mc-bridge/build.sh"
cargo build -p redox-mc-loader
cargo build -p test-mod

AGENT_LIB=$(find_library "${AGENT_LIB_BASENAME}")
MOD_LIB=$(find_library "${MOD_LIB_BASENAME}")

if [ ! -f "${AGENT_LIB}" ]; then
    echo "ERROR: Agent shared library not found at ${AGENT_LIB}"
    exit 1
fi

AGENT_PATH_NATIVE=$(to_native_path "${AGENT_LIB}")
MOD_PATH_NATIVE=$(to_native_path "${MOD_LIB}")
TEST_CLASSES_NATIVE=$(to_native_path "${ROOT_DIR}/redox-mc-bridge/build/test-classes")
CORE_JAR_NATIVE=$(to_native_path "${ROOT_DIR}/redox-mc-bridge/build/bridge-core.jar")
GAME_JAR_NATIVE=$(to_native_path "${ROOT_DIR}/redox-mc-bridge/build/bridge-game.jar")
SERVER_JAR_NATIVE=$(to_native_path "${ROOT_DIR}/versions/26.3/server-26.3.jar")

LIB_CP=""
if [ -d "${ROOT_DIR}/libraries" ]; then
    while IFS= read -r -d '' jar_file; do
        NATIVE_JAR=$(to_native_path "${jar_file}")
        LIB_CP="${LIB_CP}${CP_SEP}${NATIVE_JAR}"
    done < <(find "${ROOT_DIR}/libraries" -name "*.jar" -print0)
fi

echo ""
echo "=== Running Test 1: Client/Integrated Server ClassLoader Test ==="
java -agentpath:"${AGENT_PATH_NATIVE}" \
    -cp "${TEST_CLASSES_NATIVE}${CP_SEP}${CORE_JAR_NATIVE}${CP_SEP}${SERVER_JAR_NATIVE}" \
    net.redoxmc.test.BridgeClassLoaderTest

echo ""
echo "=== Running Test 2: Bundler URLClassLoader Simulation Test ==="
java -agentpath:"${AGENT_PATH_NATIVE}" \
    -cp "${TEST_CLASSES_NATIVE}${CP_SEP}${CORE_JAR_NATIVE}" \
    net.redoxmc.test.BundlerSimulationTest

echo ""
echo "=== Running Test 3: Real Server Bundler & Minecraft Subclass Verification ==="
BUNDLER_JAR="${MINECRAFT_BUNDLER_JAR:-}"
if [ -z "${BUNDLER_JAR}" ] || [ ! -f "${BUNDLER_JAR}" ]; then
    if [ -f "${ROOT_DIR}/versions/26.3/server.jar" ]; then
        BUNDLER_JAR="${ROOT_DIR}/versions/26.3/server.jar"
    elif [ -f "/home/ender/server.jar" ]; then
        BUNDLER_JAR="/home/ender/server.jar"
    else
        echo "[test] server.jar not found, running setup_mc_env.sh..."
        "${ROOT_DIR}/scripts/setup_mc_env.sh"
        BUNDLER_JAR="${ROOT_DIR}/versions/26.3/server.jar"
    fi
fi
BUNDLER_PATH_NATIVE=$(to_native_path "${BUNDLER_JAR}")
echo "[test] Using bundler jar: ${BUNDLER_JAR}"
SERVER_OUTPUT=$(java -agentpath:"${AGENT_PATH_NATIVE}" -jar "${BUNDLER_PATH_NATIVE}" --help 2>&1)
echo "${SERVER_OUTPUT}" | grep "Successfully bound native methods to net.redoxmc.bridge.RedoxBridge via RegisterNatives."
echo "${SERVER_OUTPUT}" | grep "Captured game classloader from first 'net/minecraft/' class: 'net/minecraft/server/Main'"
echo "${SERVER_OUTPUT}" | grep "SUCCESS: TestMinecraftSavedData instantiated: net.redoxmc.bridge.game.TestMinecraftSavedData"
echo "${SERVER_OUTPUT}" | grep "Option"

echo ""
echo "=== Running Test 4: Task 1.4 Bytecode Wrapper Hook Execution (--initSettings) ==="
INIT_TEST_DIR=$(mktemp -d 2>/dev/null || mktemp -d -t 'redox_init')
cd "${INIT_TEST_DIR}"
INIT_OUTPUT=$(java -agentpath:"${AGENT_PATH_NATIVE}" -jar "${BUNDLER_PATH_NATIVE}" --initSettings 2>&1)
cd "${ROOT_DIR}"
rm -rf "${INIT_TEST_DIR}" 2>/dev/null || true

echo "${INIT_OUTPUT}" | grep "Successfully transformed class 'net/minecraft/server/Bootstrap'"
echo "${INIT_OUTPUT}" | grep "Native RedoxBridge.onBootstrapEntry() invoked from Bootstrap.bootStrap()."
echo "${INIT_OUTPUT}" | grep "Successfully transformed class 'net/minecraft/core/registries/BuiltInRegistries'"
echo "${INIT_OUTPUT}" | grep "Native RedoxBridge.onRegisterContent() invoked before BuiltInRegistries.freeze()."
echo "${INIT_OUTPUT}" | grep "BuiltInRegistries status: open for registration (frozen=false"
echo "${INIT_OUTPUT}" | grep "Native RedoxBridge.onRegistriesFrozen() invoked after BuiltInRegistries.freeze()."
echo "${INIT_OUTPUT}" | grep "BuiltInRegistries status: sealed (frozen=true"
echo "[TEST] PASS: Bytecode wrapper method successfully intercepted and executed before vanilla logic without VerifyError."

echo ""
echo "=== Running Test 5: Task 1.6 Vertical Slice (Dynamic Mod Loading & Execution) ==="
SLICE_TEST_DIR=$(mktemp -d 2>/dev/null || mktemp -d -t 'redox_slice')
mkdir -p "${SLICE_TEST_DIR}/mods/test_mod"
cp "${ROOT_DIR}/examples/test-mod/mod.toml" "${SLICE_TEST_DIR}/mods/test_mod/"
cp "${MOD_LIB}" "${SLICE_TEST_DIR}/mods/test_mod/"

cd "${SLICE_TEST_DIR}"
MOD_SERVER_OUTPUT=$(java -agentpath:"${AGENT_PATH_NATIVE}" -jar "${BUNDLER_PATH_NATIVE}" --initSettings 2>&1)
cd "${ROOT_DIR}"
rm -rf "${SLICE_TEST_DIR}" 2>/dev/null || true

echo "${MOD_SERVER_OUTPUT}" | grep "Scanning for native mods in:"
echo "${MOD_SERVER_OUTPUT}" | grep "Resolved deterministic load order for 1 mod(s):"
echo "${MOD_SERVER_OUTPUT}" | grep "Loading native mod 'test_mod' from"
echo "${MOD_SERVER_OUTPUT}" | grep "Native Rust mod initialized successfully via redox_mod_init()!"
echo "${MOD_SERVER_OUTPUT}" | grep "Dispatching Bootstrap.bootStrap() hook to 1 loaded mod(s)..."
echo "${MOD_SERVER_OUTPUT}" | grep "Hook received: Bootstrap.bootStrap() entered! Game is bootstrapping vanilla registries."
echo "${MOD_SERVER_OUTPUT}" | grep "Dispatching BuiltInRegistries.freeze() hook to 1 loaded mod(s)..."
echo "${MOD_SERVER_OUTPUT}" | grep "Hook received: BuiltInRegistries.freeze()! Registries open for custom content."
echo "${MOD_SERVER_OUTPUT}" | grep "Registered custom item: 'testmod:ruby'"
echo "${MOD_SERVER_OUTPUT}" | grep "Registered custom block: 'testmod:ruby_block'"
echo "${MOD_SERVER_OUTPUT}" | grep "Registered custom block into BuiltInRegistries.BLOCK: 'testmod:ruby_block'"
echo "${MOD_SERVER_OUTPUT}" | grep "Registered custom item into BuiltInRegistries.ITEM: 'testmod:ruby'"
echo "${MOD_SERVER_OUTPUT}" | grep "Content registration complete: 1 custom item(s), 1 custom block(s) registered. Registry Content Hash:"
echo "${MOD_SERVER_OUTPUT}" | grep "BuiltInRegistries status: sealed (frozen=true, item_count=1660)."
echo "[TEST] PASS: Vertical slice fully operational! Native Rust mod discovered, loaded, item registered into BuiltInRegistries, and executed inside Minecraft."

echo ""
echo "=== Running Test 6: Task 4.3 Headless Server Item Validation (Console Commands) ==="
PYTHON_CMD="python3"
if ! command -v python3 >/dev/null 2>&1 && command -v python >/dev/null 2>&1; then
    PYTHON_CMD="python"
fi
AGENT_LIB="${AGENT_PATH_NATIVE}" MOD_LIB="${MOD_PATH_NATIVE}" BUNDLER_JAR="${BUNDLER_PATH_NATIVE}" "${PYTHON_CMD}" "${ROOT_DIR}/tests/test_headless_item.py"

echo ""
echo "=== Running Test 7: Task 6.1 CustomPacketPayload & ModRawPayload Codec Test ==="
java -agentpath:"${AGENT_PATH_NATIVE}" \
    -cp "${TEST_CLASSES_NATIVE}${CP_SEP}${CORE_JAR_NATIVE}${CP_SEP}${GAME_JAR_NATIVE}${CP_SEP}${SERVER_JAR_NATIVE}${LIB_CP}" \
    net.redoxmc.test.CustomPayloadTest

echo ""
echo "=== Running Test 8: Task 6.2 ModHandshakeConfigurationTask & Registry Hash Verification ==="
java -agentpath:"${AGENT_PATH_NATIVE}" \
    -cp "${TEST_CLASSES_NATIVE}${CP_SEP}${CORE_JAR_NATIVE}${CP_SEP}${GAME_JAR_NATIVE}${CP_SEP}${SERVER_JAR_NATIVE}${LIB_CP}" \
    net.redoxmc.test.ModHandshakeTest

echo ""
echo "=== Running Test 9: Task 6.3 Typed Network Channels (Ping/Pong with Sender Identity) ==="
NETWORK_TEST_DIR=$(mktemp -d 2>/dev/null || mktemp -d -t 'redox_network')
mkdir -p "${NETWORK_TEST_DIR}/mods/test_mod"
cp "${ROOT_DIR}/examples/test-mod/mod.toml" "${NETWORK_TEST_DIR}/mods/test_mod/"
cp "${MOD_LIB}" "${NETWORK_TEST_DIR}/mods/test_mod/"

cd "${NETWORK_TEST_DIR}"
java -agentpath:"${AGENT_PATH_NATIVE}" \
    -cp "${TEST_CLASSES_NATIVE}${CP_SEP}${CORE_JAR_NATIVE}${CP_SEP}${GAME_JAR_NATIVE}${CP_SEP}${SERVER_JAR_NATIVE}${LIB_CP}" \
    net.redoxmc.test.NetworkChannelTest
cd "${ROOT_DIR}"
rm -rf "${NETWORK_TEST_DIR}" 2>/dev/null || true
echo "[TEST] PASS: Typed Rust network channels transported ping/pong/ack with sender identity."

echo ""
echo ">>> ALL INTEGRATION TESTS PASSED! <<<"

