package net.redoxmc.bridge.client;

import net.redoxmc.bridge.game.RedoxGameBridgeBase;

/**
 * Client-only bridge module.
 * Separated into a distinct module so that dedicated server runs never load it,
 * preventing NoClassDefFoundError for client-only dependencies.
 */
public class RedoxClientBridge extends RedoxGameBridgeBase {
    public RedoxClientBridge() {
        super("redox-mc-bridge-client");
    }

    public static boolean isClientSupported() {
        return true;
    }
}

