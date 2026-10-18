package net.rustmc.bridge.client;

import net.rustmc.bridge.game.RustGameBridgeBase;

/**
 * Client-only bridge module.
 * Separated into a distinct module so that dedicated server runs never load it,
 * preventing NoClassDefFoundError for client-only dependencies.
 */
public class RustClientBridge extends RustGameBridgeBase {
    public RustClientBridge() {
        super("rustmc-bridge-client");
    }

    public static boolean isClientSupported() {
        return true;
    }
}
