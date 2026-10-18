#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_DIR="${SCRIPT_DIR}/build"

mkdir -p "${BUILD_DIR}/core-classes" "${BUILD_DIR}/game-classes" "${BUILD_DIR}/client-classes"

echo "[redox-mc-bridge] Compiling bridge-core..."
javac -d "${BUILD_DIR}/core-classes" \
    "${SCRIPT_DIR}/bridge-core/src/main/java/net/redoxmc/bridge/RedoxBridge.java"
jar cf "${BUILD_DIR}/bridge-core.jar" -C "${BUILD_DIR}/core-classes" .

case "$(uname -s)" in
    CYGWIN*|MINGW*|MSYS*) CP_SEP=";" ;;
    *) CP_SEP=":" ;;
esac

to_native_path() {
    if command -v cygpath >/dev/null 2>&1; then
        cygpath -w "$1"
    else
        echo "$1"
    fi
}

echo "[redox-mc-bridge] Compiling bridge-game against Minecraft server jar..."
MC_SERVER_JAR="${SCRIPT_DIR}/../versions/26.3/server-26.3.jar"
if [ ! -f "${MC_SERVER_JAR}" ]; then
    "${SCRIPT_DIR}/../scripts/setup_mc_env.sh"
fi

CORE_JAR_NATIVE=$(to_native_path "${BUILD_DIR}/bridge-core.jar")
MC_SERVER_JAR_NATIVE=$(to_native_path "${MC_SERVER_JAR}")
GAME_JAR_NATIVE=$(to_native_path "${BUILD_DIR}/bridge-game.jar")

LIB_CP=""
if [ -d "${SCRIPT_DIR}/../libraries" ]; then
    while IFS= read -r -d '' jar_file; do
        NATIVE_JAR=$(to_native_path "${jar_file}")
        LIB_CP="${LIB_CP}${CP_SEP}${NATIVE_JAR}"
    done < <(find "${SCRIPT_DIR}/../libraries" -name "*.jar" -print0)
fi

javac -cp "${CORE_JAR_NATIVE}${CP_SEP}${MC_SERVER_JAR_NATIVE}${LIB_CP}" -d "${BUILD_DIR}/game-classes" \
    "${SCRIPT_DIR}/bridge-game/src/main/java/net/redoxmc/bridge/game/RedoxGameBridgeBase.java" \
    "${SCRIPT_DIR}/bridge-game/src/main/java/net/redoxmc/bridge/game/RedoxGameBridge.java" \
    "${SCRIPT_DIR}/bridge-game/src/main/java/net/redoxmc/bridge/game/TestMinecraftSavedData.java" \
    "${SCRIPT_DIR}/bridge-game/src/main/java/net/redoxmc/bridge/event/ServerEvents.java"

jar cf "${BUILD_DIR}/bridge-game.jar" -C "${BUILD_DIR}/game-classes" .

echo "[redox-mc-bridge] Compiling bridge-client..."
javac -cp "${CORE_JAR_NATIVE}${CP_SEP}${GAME_JAR_NATIVE}" -d "${BUILD_DIR}/client-classes" \
    "${SCRIPT_DIR}/bridge-client/src/main/java/net/redoxmc/bridge/client/RedoxClientBridge.java"
jar cf "${BUILD_DIR}/bridge-client.jar" -C "${BUILD_DIR}/client-classes" .

echo "[redox-mc-bridge] Compiling test classes..."
mkdir -p "${BUILD_DIR}/test-classes" "${BUILD_DIR}/test-game-classes"
javac -cp "${CORE_JAR_NATIVE}" -d "${BUILD_DIR}/test-classes" \
    "${SCRIPT_DIR}/src/test/java/net/minecraft/client/TestClientEntry.java" \
    "${SCRIPT_DIR}/src/test/java/net/minecraft/server/TestServerEntry.java" \
    "${SCRIPT_DIR}/src/test/java/net/redoxmc/test/BridgeClassLoaderTest.java" \
    "${SCRIPT_DIR}/src/test/java/net/redoxmc/test/BundlerSimulationTest.java"
javac -d "${BUILD_DIR}/test-game-classes" \
    "${SCRIPT_DIR}/src/test/java/net/minecraft/server/TestBundledMain.java"

echo "[redox-mc-bridge] Build completed successfully."
ls -lh "${BUILD_DIR}"/*.jar

