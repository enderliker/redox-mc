package net.rustmc.bridge.game;

/**
 * Superclass for game bridge components.
 * Loaded into the Minecraft game classloader BEFORE child classes.
 */
public class RustGameBridgeBase {
    private final String moduleName;

    public RustGameBridgeBase(String moduleName) {
        this.moduleName = moduleName;
    }

    public String getModuleName() {
        return this.moduleName;
    }
}
