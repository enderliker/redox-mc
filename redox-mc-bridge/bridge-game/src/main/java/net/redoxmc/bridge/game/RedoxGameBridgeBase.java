package net.redoxmc.bridge.game;


/**
 * Superclass for game bridge components.
 * Loaded into the Minecraft game classloader BEFORE child classes.
 */
public class RedoxGameBridgeBase {
    private final String moduleName;

    public RedoxGameBridgeBase(String moduleName) {
        this.moduleName = moduleName;
    }

    public String getModuleName() {
        return this.moduleName;
    }
}

