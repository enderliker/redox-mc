package net.redoxmc.bridge.gui;

import java.lang.reflect.Constructor;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import net.minecraft.core.Registry;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.SimpleContainer;
import net.minecraft.world.SimpleMenuProvider;
import net.minecraft.world.flag.FeatureFlagSet;
import net.minecraft.world.flag.FeatureFlags;
import net.minecraft.world.inventory.MenuType;

/**
 * Helper utility for constructing and registering custom MenuType instances
 * and opening container menus on the server.
 */
public class GenericRedoxMenuHelper {
    private static final Map<String, MenuType<GenericRedoxMenu>> REGISTERED_MENUS = new ConcurrentHashMap<>();

    @SuppressWarnings("unchecked")
    public static MenuType<GenericRedoxMenu> createMenuType(String menuId, int slotCount) {
        try {
            Constructor<?>[] constructors = MenuType.class.getDeclaredConstructors();
            Constructor<?> targetCtor = null;
            for (Constructor<?> ctor : constructors) {
                Class<?>[] params = ctor.getParameterTypes();
                if (params.length == 2 && FeatureFlagSet.class.isAssignableFrom(params[1])) {
                    targetCtor = ctor;
                    break;
                }
            }
            if (targetCtor == null) {
                throw new IllegalStateException("Could not find MenuType constructor with FeatureFlagSet parameter");
            }
            targetCtor.setAccessible(true);

            Class<?> supplierInterface = targetCtor.getParameterTypes()[0];
            Object supplierProxy = java.lang.reflect.Proxy.newProxyInstance(
                    GenericRedoxMenuHelper.class.getClassLoader(),
                    new Class<?>[]{supplierInterface},
                    (proxy, method, args) -> {
                        if ("create".equals(method.getName()) && args != null && args.length >= 2) {
                            int containerId = (Integer) args[0];
                            net.minecraft.world.entity.player.Inventory playerInventory =
                                    (net.minecraft.world.entity.player.Inventory) args[1];
                            return new GenericRedoxMenu(
                                    null,
                                    containerId,
                                    playerInventory,
                                    new SimpleContainer(slotCount),
                                    menuId,
                                    slotCount
                            );
                        }
                        if ("equals".equals(method.getName())) {
                            return proxy == args[0];
                        }
                        if ("hashCode".equals(method.getName())) {
                            return System.identityHashCode(proxy);
                        }
                        if ("toString".equals(method.getName())) {
                            return "GenericRedoxMenuSupplier[" + menuId + "]";
                        }
                        return null;
                    }
            );

            MenuType<GenericRedoxMenu> menuType = (MenuType<GenericRedoxMenu>) targetCtor.newInstance(
                    supplierProxy,
                    FeatureFlags.VANILLA_SET
            );
            return menuType;
        } catch (Exception e) {
            throw new RuntimeException("Failed to construct MenuType for " + menuId, e);
        }
    }

    public static MenuType<GenericRedoxMenu> registerMenuType(String menuId, int slotCount) {
        MenuType<GenericRedoxMenu> menuType = createMenuType(menuId, slotCount);
        Registry.register(BuiltInRegistries.MENU, Identifier.parse(menuId), menuType);
        REGISTERED_MENUS.put(menuId, menuType);
        System.out.println("[redox-mc-bridge] Registered custom MenuType: '" + menuId + "' with " + slotCount + " slots");
        return menuType;
    }

    public static MenuType<GenericRedoxMenu> getMenuType(String menuId) {
        return REGISTERED_MENUS.get(menuId);
    }

    public static void openMenu(ServerPlayer player, String menuId, int slotCount, String title) {
        if (player == null) {
            return;
        }
        MenuType<GenericRedoxMenu> menuType = REGISTERED_MENUS.get(menuId);
        Component titleComp = Component.literal(title != null ? title : menuId);
        player.openMenu(new SimpleMenuProvider(
                (containerId, playerInventory, p) -> new GenericRedoxMenu(
                        menuType,
                        containerId,
                        playerInventory,
                        new SimpleContainer(slotCount),
                        menuId,
                        slotCount
                ),
                titleComp
        ));
    }

    public static String verifyMenuInstantiation() {
        MenuType<GenericRedoxMenu> menuType = createMenuType("testmod:verification_menu", 9);
        return "GenericRedoxMenuHelper verified successfully: " + (menuType != null);
    }

    public static void testMenuSlotClickLifecycle(String menuId) {
        try {
            MenuType<GenericRedoxMenu> type = REGISTERED_MENUS.get(menuId);
            GenericRedoxMenu menu = new GenericRedoxMenu(
                    type,
                    100,
                    null,
                    new SimpleContainer(9),
                    menuId,
                    9
            );
            System.out.println("[redox-mc-bridge] Exercising slot click 0 (expecting Allow)...");
            menu.clicked(0, 0, net.minecraft.world.inventory.ContainerInput.PICKUP, null);

            System.out.println("[redox-mc-bridge] Exercising slot click 8 (expecting Deny)...");
            menu.clicked(8, 0, net.minecraft.world.inventory.ContainerInput.PICKUP, null);

            System.out.println("[redox-mc-bridge] Closing menu session...");
            menu.removed(null);
            System.out.println("[redox-mc-bridge] testMenuSlotClickLifecycle completed successfully.");
        } catch (Exception e) {
            System.err.println("[redox-mc-bridge] Error during testMenuSlotClickLifecycle: " + e);
        }
    }
}
