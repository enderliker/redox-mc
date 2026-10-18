#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
VERSIONS_DIR="${ROOT_DIR}/versions/26.3"
SERVER_INNER="${VERSIONS_DIR}/server-26.3.jar"

if [ -f "${SERVER_INNER}" ]; then
    echo "[setup] Minecraft 26.3 server jar already exists at: ${SERVER_INNER}"
    exit 0
fi

mkdir -p "${VERSIONS_DIR}"

BUNDLER_JAR="${MINECRAFT_BUNDLER_JAR:-}"
if [ -z "${BUNDLER_JAR}" ] && [ -f "/home/ender/server.jar" ]; then
    BUNDLER_JAR="/home/ender/server.jar"
fi

CLEANUP_BUNDLER=false
if [ -z "${BUNDLER_JAR}" ] || [ ! -f "${BUNDLER_JAR}" ]; then
    echo "[setup] Downloading official Minecraft 26.3 server jar from Mojang piston..."
    BUNDLER_JAR="/tmp/mc-server-26.3.jar"
    curl -sSL "https://piston-data.mojang.com/v1/objects/33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c/server.jar" -o "${BUNDLER_JAR}"
    CLEANUP_BUNDLER=true
fi

echo "[setup] Extracting server-26.3.jar from bundler..."
unzip -p "${BUNDLER_JAR}" "META-INF/versions/26.3/server-26.3.jar" > "${SERVER_INNER}"

if [ "${CLEANUP_BUNDLER}" = true ] && [ -f "${BUNDLER_JAR}" ]; then
    rm -f "${BUNDLER_JAR}"
fi

echo "[setup] Minecraft 26.3 server jar ready at ${SERVER_INNER} ($(ls -lh "${SERVER_INNER}" | awk '{print $5}'))"
