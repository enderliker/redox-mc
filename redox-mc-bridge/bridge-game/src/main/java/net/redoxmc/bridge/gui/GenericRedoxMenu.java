package net.redoxmc.bridge.gui;

import net.minecraft.world.Container;
import net.minecraft.world.SimpleContainer;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.inventory.AbstractContainerMenu;
import net.minecraft.world.inventory.ContainerInput;
import net.minecraft.world.inventory.MenuType;
import net.minecraft.world.inventory.Slot;
import net.minecraft.world.item.ItemStack;
import net.redoxmc.bridge.RedoxBridge;

/**
 * Generic extensible container menu for Redox-MC mods.
 * Intercepts slot clicks and delegates lifecycle callbacks to native Rust.
 */
public class GenericRedoxMenu extends AbstractContainerMenu {
    private final String menuId;
    private final int slotCount;
    private final Container container;

    public GenericRedoxMenu(
            MenuType<?> type,
            int containerId,
            Inventory playerInventory
    ) {
        this(type, containerId, playerInventory, new SimpleContainer(9), "unknown", 9);
    }

    public GenericRedoxMenu(
            MenuType<?> type,
            int containerId,
            Inventory playerInventory,
            Container container,
            String menuId,
            int slotCount
    ) {
        super(type, containerId);
        this.menuId = (menuId != null) ? menuId : "unknown";
        this.slotCount = slotCount;
        this.container = (container != null) ? container : new SimpleContainer(slotCount);

        if (container != null && playerInventory != null && playerInventory.player != null) {
            container.startOpen(playerInventory.player);
        }

        // Add custom container slots
        for (int i = 0; i < slotCount; i++) {
            int col = i % 9;
            int row = i / 9;
            this.addSlot(new Slot(this.container, i, 8 + col * 18, 18 + row * 18));
        }

        // Add standard 36 player inventory slots (3 rows of 9 + 1 hotbar row)
        if (playerInventory != null) {
            int rows = (slotCount + 8) / 9;
            int playerInvY = 18 + Math.max(3, rows) * 18 + 14;
            this.addStandardInventorySlots(playerInventory, 8, playerInvY);
        }

        String playerName = (playerInventory != null && playerInventory.player != null && playerInventory.player.getName() != null)
                ? playerInventory.player.getName().getString()
                : "";
        RedoxBridge.onMenuOpen(this.menuId, this.containerId, playerName);
    }

    public String getMenuId() {
        return this.menuId;
    }

    public int getSlotCount() {
        return this.slotCount;
    }

    public Container getContainer() {
        return this.container;
    }

    @Override
    public void clicked(int slotIndex, int button, ContainerInput input, Player player) {
        String playerName = (player != null && player.getName() != null)
                ? player.getName().getString()
                : "";
        int clickTypeId = (input != null) ? input.id() : 0;
        int action = RedoxBridge.onMenuSlotClick(
                this.menuId,
                this.containerId,
                slotIndex,
                button,
                clickTypeId,
                playerName
        );
        if (action == 1) { // 1 = DENY
            this.broadcastFullState();
            return;
        }
        if (player != null) {
            super.clicked(slotIndex, button, input, player);
        }
    }

    @Override
    public boolean stillValid(Player player) {
        if (player == null) {
            return true;
        }
        return this.container == null || this.container.stillValid(player);
    }

    @Override
    public ItemStack quickMoveStack(Player player, int slotIndex) {
        ItemStack itemStack = ItemStack.EMPTY;
        if (slotIndex < 0 || slotIndex >= this.slots.size()) {
            return itemStack;
        }
        Slot slot = this.slots.get(slotIndex);
        if (slot != null && slot.hasItem()) {
            ItemStack slotStack = slot.getItem();
            itemStack = slotStack.copy();

            if (slotIndex < this.slotCount) {
                // Moving from container into player inventory
                if (!this.moveItemStackTo(slotStack, this.slotCount, this.slots.size(), true)) {
                    return ItemStack.EMPTY;
                }
            } else {
                // Moving from player inventory into container
                if (!this.moveItemStackTo(slotStack, 0, this.slotCount, false)) {
                    return ItemStack.EMPTY;
                }
            }

            if (slotStack.isEmpty()) {
                slot.setByPlayer(ItemStack.EMPTY);
            } else {
                slot.setChanged();
            }

            if (slotStack.getCount() == itemStack.getCount()) {
                return ItemStack.EMPTY;
            }

            slot.onTake(player, slotStack);
        }
        return itemStack;
    }

    @Override
    public void removed(Player player) {
        if (player != null) {
            super.removed(player);
        }
        if (this.container != null && player != null) {
            this.container.stopOpen(player);
        }
        String playerName = (player != null && player.getName() != null)
                ? player.getName().getString()
                : "";
        RedoxBridge.onMenuClose(this.menuId, this.containerId, playerName);
    }
}
