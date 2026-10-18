package net.rustmc.bridge.game;

import net.minecraft.world.item.Item;

/**
 * Real Minecraft subclass to verify that bridge-game classes defined into the
 * captured game URLClassLoader can resolve and inherit from Minecraft types
 * without NoClassDefFoundError.
 */
public class TestMinecraftItem extends Item {
    public TestMinecraftItem(Item.Properties properties) {
        super(properties);
    }

    public static TestMinecraftItem createTestInstance() {
        return new TestMinecraftItem(new Item.Properties());
    }

    public static String getTestStatus() {
        TestMinecraftItem instance = createTestInstance();
        return "SUCCESS: TestMinecraftItem instantiated: " + instance.getClass().getName()
            + " (superclass=" + instance.getClass().getSuperclass().getName() + ")";
    }
}
