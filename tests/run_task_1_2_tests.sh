#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
AGENT_SO="${ROOT_DIR}/target/debug/libredox_mc_loader.so"

echo "=== Building redox-mc-bridge and compiling agent ==="
"${ROOT_DIR}/redox-mc-bridge/build.sh"
cargo build -p redox-mc-loader

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
echo ">>> ALL TASK 1.2 TESTS PASSED! <<<"

