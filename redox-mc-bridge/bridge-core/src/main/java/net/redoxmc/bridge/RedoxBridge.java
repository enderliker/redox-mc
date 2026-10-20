package net.redoxmc.bridge;

/**
 * Core bridge between the JVM and Redox-MC native mod loader.

 * This class has ZERO dependencies on Minecraft classes and is added to the
 * JVM Bootstrap ClassLoader search path, making it universally accessible.
 */
public final class RedoxBridge {
    private static volatile boolean initialized = false;

    // Private constructor to prevent instantiation
    private RedoxBridge() {}

    /**
     * Initializes the native bridge and triggers definition of bridge-game classes
     * onto the captured game classloader.
     */
    public static native void init();

    /**
     * Hook called at the entry of net.minecraft.server.Bootstrap.bootStrap().
     */
    public static native void onBootstrapEntry();

    /**
     * Hook called right before BuiltInRegistries.freeze() seals the registries.
     */
    public static native void onRegisterContent();

    /**
     * Hook called immediately after BuiltInRegistries.freeze$original() has sealed the registries.
     */
    public static native void onRegistriesFrozen();

    /**
     * Hook called every server tick loop with the current tick count.
     */
    public static native void onServerTick(long tickCount);

    /**
     * Hook called on incoming player chat messages.
     * Returns true to allow message delivery, false to cancel/drop the message.
     */
    public static native boolean onServerChat(String playerName, String message);

    /**
     * Hook called when a player uses an item (right-click in air).
     * Returns 0 for PASS, 1 for SUCCESS, 2 for CONSUME, 3 for FAIL.
     */
    public static native int onItemUse(String itemId, int hand);

    /**
     * Hook called when a player uses an item on a block.
     * Returns 0 for PASS, 1 for SUCCESS, 2 for CONSUME, 3 for FAIL.
     */
    public static native int onItemUseOn(String itemId, int x, int y, int z, int face, int hand);

    /**
     * Hook called when an item finishes being used (e.g. food/potion finished consuming).
     */
    public static native void onItemFinishUsing(String itemId);

    /**
     * Returns true if the game classloader (first class in net/minecraft/*) has been captured.
     */
    public static native boolean isClassLoaderCaptured();

    /**
     * Returns the captured game ClassLoader, or null if not yet captured.
     */
    public static native ClassLoader getCapturedClassLoader();

    /**
     * Helper to test DefineClass error handling without crashing the JVM.
     * Returns true if defined successfully, false if an error occurred and was handled cleanly.
     */
    public static native boolean testDefineClass(String className, byte[] classBytes);

    /**
     * Ensures native initialization runs exactly once.
     */
    public static synchronized void ensureInitialized() {
        if (!initialized) {
            init();
            initialized = true;
        }
    }
}
