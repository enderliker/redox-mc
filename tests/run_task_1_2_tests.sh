#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
AGENT_SO="${ROOT_DIR}/target/debug/libredox_mc_loader.so"

echo "=== Building redox-mc-bridge and compiling agent ==="
"${ROOT_DIR}/redox-mc-bridge/build.sh"
cargo build -p redox-mc-loader
cargo build -p test-mod

if [ ! -f "${AGENT_SO}" ]; then
    echo "ERROR: Agent shared library not found at ${AGENT_SO}"
    exit 1
fi

echo ""
echo "=== Running Test 1: Client/Integrated Server ClassLoader Test ==="
java -agentpath:"${AGENT_SO}" \
    -cp "${ROOT_DIR}/redox-mc-bridge/build/test-classes:${ROOT_DIR}/redox-mc-bridge/build/bridge-core.jar:${ROOT_DIR}/versions/26.3/server-26.3.jar" \
    net.redoxmc.test.BridgeClassLoaderTest

echo ""
echo "=== Running Test 2: Bundler URLClassLoader Simulation Test ==="
java -agentpath:"${AGENT_SO}" \
    -cp "${ROOT_DIR}/redox-mc-bridge/build/test-classes:${ROOT_DIR}/redox-mc-bridge/build/bridge-core.jar" \
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
echo "[test] Using bundler jar: ${BUNDLER_JAR}"
SERVER_OUTPUT=$(java -agentpath:"${AGENT_SO}" -jar "${BUNDLER_JAR}" --help 2>&1)
echo "${SERVER_OUTPUT}" | grep "Successfully bound native methods to net.redoxmc.bridge.RedoxBridge via RegisterNatives."
echo "${SERVER_OUTPUT}" | grep "Captured game classloader from first 'net/minecraft/' class: 'net/minecraft/server/Main'"
echo "${SERVER_OUTPUT}" | grep "SUCCESS: TestMinecraftSavedData instantiated: net.redoxmc.bridge.game.TestMinecraftSavedData"
echo "${SERVER_OUTPUT}" | grep "Option"

echo ""
echo "=== Running Test 4: Task 1.4 Bytecode Wrapper Hook Execution (--initSettings) ==="
INIT_TEST_DIR=$(mktemp -d /tmp/redox_init_test_XXXXXX)
cd "${INIT_TEST_DIR}"
INIT_OUTPUT=$(java -agentpath:"${AGENT_SO}" -jar "${BUNDLER_JAR}" --initSettings 2>&1)
cd "${ROOT_DIR}"
rm -rf "${INIT_TEST_DIR}"

echo "${INIT_OUTPUT}" | grep "Successfully transformed class 'net/minecraft/server/Bootstrap'"
echo "${INIT_OUTPUT}" | grep "Native RedoxBridge.onBootstrapEntry() invoked from Bootstrap.bootStrap()."
echo "${INIT_OUTPUT}" | grep "Successfully transformed class 'net/minecraft/core/registries/BuiltInRegistries'"
echo "${INIT_OUTPUT}" | grep "Native RedoxBridge.onRegisterContent() invoked before BuiltInRegistries.freeze()."
echo "[TEST] PASS: Bytecode wrapper method successfully intercepted and executed before vanilla logic without VerifyError."

echo ""
echo "=== Running Test 5: Task 1.6 Vertical Slice (Dynamic Mod Loading & Execution) ==="
SLICE_TEST_DIR=$(mktemp -d /tmp/redox_slice_test_XXXXXX)
mkdir -p "${SLICE_TEST_DIR}/mods/test_mod"
cp "${ROOT_DIR}/examples/test-mod/mod.toml" "${SLICE_TEST_DIR}/mods/test_mod/"
cp "${ROOT_DIR}/target/debug/libtest_mod.so" "${SLICE_TEST_DIR}/mods/test_mod/"

cd "${SLICE_TEST_DIR}"
MOD_SERVER_OUTPUT=$(java -agentpath:"${AGENT_SO}" -jar "${BUNDLER_JAR}" --initSettings 2>&1)
cd "${ROOT_DIR}"
rm -rf "${SLICE_TEST_DIR}"

echo "${MOD_SERVER_OUTPUT}" | grep "Scanning for native mods in:"
echo "${MOD_SERVER_OUTPUT}" | grep "Resolved deterministic load order for 1 mod(s):"
echo "${MOD_SERVER_OUTPUT}" | grep "Loading native mod 'test_mod' from"
echo "${MOD_SERVER_OUTPUT}" | grep "Native Rust mod initialized successfully via redox_mod_init()!"
echo "${MOD_SERVER_OUTPUT}" | grep "Dispatching Bootstrap.bootStrap() hook to 1 loaded mod(s)..."
echo "${MOD_SERVER_OUTPUT}" | grep "Hook received: Bootstrap.bootStrap() entered! Game is bootstrapping vanilla registries."
echo "[TEST] PASS: Vertical slice fully operational! Native Rust mod discovered, loaded, and executed inside Minecraft."

echo ""
echo ">>> ALL INTEGRATION TESTS PASSED! <<<"

