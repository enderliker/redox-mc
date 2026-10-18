package net.rustmc.bridge.game;

/**
 * Common game bridge implementation extending RustGameBridgeBase.
 * Verifies that inheritance works within the dynamically defined classes.
 */
public class RustGameBridge extends RustGameBridgeBase {
    public RustGameBridge() {
        super("rustmc-bridge-game");
    }

    public static String getVersion() {
        return "26.3-0.1.0";
    }
}
