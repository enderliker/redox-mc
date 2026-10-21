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
     * Hook called when a player interacts with a block without a held item.
     * Returns 0 for PASS, 1 for SUCCESS, 2 for CONSUME, 3 for FAIL.
     */
    public static native int onBlockUseWithoutItem(String blockId, int x, int y, int z, int direction);

    /**
     * Hook called when a custom mod network payload is received.
     *
     * @param namespace channel namespace (e.g. "testmod")
     * @param path channel path (e.g. "data")
     * @param data raw payload byte content
     */
    public static native void onModCustomPayload(String namespace, String path, byte[] data);

    /**
     * Returns the computed registry content hash, or empty string if none.
     */
    public static native String getRegistryHash();

    /**
     * Returns the array of loaded mod IDs on this instance.
     */
    public static native String[] getLoadedModIds();

    /**
     * Returns the array of loaded mod versions corresponding to getLoadedModIds().
     */
    public static native String[] getLoadedModVersions();

    /**
     * Validates client handshake credentials against server requirements.
     *
     * @param clientHash the registry content hash reported by the client
     * @param clientModIds array of mod IDs installed on client
     * @param clientModVersions array of mod versions installed on client
     * @return null if handshake is valid, or error reason string if invalid
     */
    public static native String validateClientHandshake(String clientHash, String[] clientModIds, String[] clientModVersions);

    /**
     * Hook called when an entity steps onto a block.
     */
    public static native void onBlockStepOn(String blockId, int x, int y, int z);

    /**
     * Hook called when a block is destroyed.
     */
    public static native void onBlockDestroy(String blockId, int x, int y, int z);

    /**
     * Retrieves a virtual resource from the native loader.
     *
     * @param type "assets" (CLIENT_RESOURCES) or "data" (SERVER_DATA)
     * @param namespace e.g. "testmod"
     * @param path e.g. "loot_table/blocks/ruby_block.json"
     * @return raw byte content or null if not registered
     */
    public static native byte[] getVirtualResource(String type, String namespace, String path);

    /**
     * Lists virtual resources under a directory prefix.
     *
     * @param type "assets" or "data"
     * @param namespace e.g. "testmod"
     * @param directory e.g. "recipes"
     * @return array of relative resource paths under the directory
     */
    public static native String[] listVirtualResources(String type, String namespace, String directory);

    /**
     * Returns all namespaces registered for a given pack type.
     *
     * @param type "assets" or "data"
     * @return array of namespace strings
     */
    public static native String[] getVirtualNamespaces(String type);

    /**
     * Retrieves root virtual pack resources (e.g. "pack.mcmeta").
     *
     * @param path e.g. "pack.mcmeta"
     * @return raw byte content or null if not registered
     */
    public static native byte[] getVirtualRootResource(String path);

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
