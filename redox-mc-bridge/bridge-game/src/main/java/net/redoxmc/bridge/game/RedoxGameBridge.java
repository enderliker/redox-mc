package net.redoxmc.bridge.game;

/**
 * Common game bridge implementation extending RedoxGameBridgeBase.
 * Verifies that inheritance works within the dynamically defined classes.
 */
public class RedoxGameBridge extends RedoxGameBridgeBase {
    public RedoxGameBridge() {
        super("redox-mc-bridge-game");
    }


    public static String getVersion() {
        return "26.3-0.1.0";
    }
}

