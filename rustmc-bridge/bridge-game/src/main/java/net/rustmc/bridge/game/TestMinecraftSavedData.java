package net.rustmc.bridge.game;

import net.minecraft.world.level.saveddata.SavedData;

/**
 * Real Minecraft subclass to verify that bridge-game classes defined into the
 * captured game URLClassLoader can resolve and inherit from Minecraft types
 * without NoClassDefFoundError.
 */
public class TestMinecraftSavedData extends SavedData {
    public TestMinecraftSavedData() {
        super();
        this.setDirty(true);
    }

    public static TestMinecraftSavedData createTestInstance() {
        return new TestMinecraftSavedData();
    }

    public static String getTestStatus() {
        TestMinecraftSavedData instance = createTestInstance();
        return "SUCCESS: TestMinecraftSavedData instantiated: " + instance.getClass().getName()
            + " (superclass=" + instance.getClass().getSuperclass().getName()
            + ", isDirty=" + instance.isDirty() + ")";
    }
}
