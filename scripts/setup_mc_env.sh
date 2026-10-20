#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
VERSIONS_DIR="${ROOT_DIR}/versions/26.3"
BUNDLER_TARGET="${VERSIONS_DIR}/server.jar"
SERVER_INNER="${VERSIONS_DIR}/server-26.3.jar"
LIBRARIES_DIR="${ROOT_DIR}/libraries"

mkdir -p "${VERSIONS_DIR}"

# 1. Ensure bundler server.jar is present
if [ ! -f "${BUNDLER_TARGET}" ]; then
    if [ -n "${MINECRAFT_BUNDLER_JAR:-}" ] && [ -f "${MINECRAFT_BUNDLER_JAR}" ]; then
        echo "[setup] Copying bundler jar from ${MINECRAFT_BUNDLER_JAR} to ${BUNDLER_TARGET}..."
        cp "${MINECRAFT_BUNDLER_JAR}" "${BUNDLER_TARGET}"
    elif [ -f "/home/ender/server.jar" ]; then
        echo "[setup] Copying local /home/ender/server.jar to ${BUNDLER_TARGET}..."
        cp "/home/ender/server.jar" "${BUNDLER_TARGET}"
    else
        echo "[setup] Downloading official Minecraft 26.3 server bundler jar from Mojang piston..."
        curl -sSL "https://piston-data.mojang.com/v1/objects/33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c/server.jar" -o "${BUNDLER_TARGET}"
    fi
fi

# 2. Ensure inner server-26.3.jar is extracted
if [ ! -f "${SERVER_INNER}" ]; then
    echo "[setup] Extracting server-26.3.jar from ${BUNDLER_TARGET}..."
    if command -v unzip >/dev/null 2>&1; then
        unzip -p "${BUNDLER_TARGET}" "META-INF/versions/26.3/server-26.3.jar" > "${SERVER_INNER}"
    else
        EXTRACT_TMP=$(mktemp -d 2>/dev/null || mktemp -d -t 'redox_extract')
        (cd "${EXTRACT_TMP}" && jar xf "${BUNDLER_TARGET}" "META-INF/versions/26.3/server-26.3.jar")
        cp "${EXTRACT_TMP}/META-INF/versions/26.3/server-26.3.jar" "${SERVER_INNER}"
        rm -rf "${EXTRACT_TMP}"
    fi
fi

# 3. Ensure bundler libraries are extracted
if [ ! -d "${LIBRARIES_DIR}" ] || [ -z "$(find "${LIBRARIES_DIR}" -name "*.jar" -print -quit 2>/dev/null)" ]; then
    echo "[setup] Extracting bundler libraries from ${BUNDLER_TARGET} into ${LIBRARIES_DIR}..."
    mkdir -p "${LIBRARIES_DIR}"
    EXTRACT_TMP=$(mktemp -d 2>/dev/null || mktemp -d -t 'redox_libs')
    if command -v unzip >/dev/null 2>&1; then
        unzip -q -o "${BUNDLER_TARGET}" "META-INF/libraries/*" -d "${EXTRACT_TMP}"
    else
        (cd "${EXTRACT_TMP}" && jar xf "${BUNDLER_TARGET}" "META-INF/libraries")
    fi
    if [ -d "${EXTRACT_TMP}/META-INF/libraries" ]; then
        cp -r "${EXTRACT_TMP}/META-INF/libraries/"* "${LIBRARIES_DIR}/"
    fi
    rm -rf "${EXTRACT_TMP}"
fi

echo "[setup] Minecraft 26.3 environment ready:"
echo " - Bundler jar: ${BUNDLER_TARGET} ($(ls -lh "${BUNDLER_TARGET}" | awk '{print $5}'))"
echo " - Inner jar:   ${SERVER_INNER} ($(ls -lh "${SERVER_INNER}" | awk '{print $5}'))"
echo " - Libraries:   ${LIBRARIES_DIR} ($(find "${LIBRARIES_DIR}" -name "*.jar" | wc -l) jars)"
