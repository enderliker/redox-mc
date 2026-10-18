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
