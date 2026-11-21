package net.redoxmc.bridge.block;

import net.minecraft.core.BlockPos;
import net.minecraft.network.chat.Component;
import net.minecraft.world.ContainerHelper;
import net.minecraft.world.MenuProvider;
import net.minecraft.world.SimpleContainer;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.inventory.AbstractContainerMenu;
import net.minecraft.world.inventory.MenuType;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.entity.BlockEntityType;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.storage.ValueInput;
import net.minecraft.world.level.storage.ValueOutput;
import net.redoxmc.bridge.RedoxBridge;
import net.redoxmc.bridge.gui.GenericRedoxMenu;
import net.redoxmc.bridge.gui.GenericRedoxMenuHelper;

public class GenericRedoxBlockEntity extends BlockEntity implements MenuProvider {
    public static final int FLAG_HAS_TICK = 1 << 0;
    public static final int FLAG_HAS_PERSISTENCE = 1 << 1;
    public static final int FLAG_HAS_CONTAINER = 1 << 2;

    private final String blockEntityId;
    private final int behaviorFlags;
    private final int slotCount;
    private final String menuId;
    private final String containerTitle;
    private final SimpleContainer container;

    public GenericRedoxBlockEntity(
            BlockEntityType<?> type,
            BlockPos pos,
            BlockState state,
            String blockEntityId,
            int behaviorFlags,
            int slotCount,
            String menuId,
            String containerTitle
    ) {
        super(type, pos, state);
        this.blockEntityId = (blockEntityId != null) ? blockEntityId : "unknown";
        this.behaviorFlags = behaviorFlags;
        this.slotCount = (slotCount > 0) ? slotCount : 9;
        this.menuId = (menuId != null && !menuId.isEmpty()) ? menuId : this.blockEntityId;
        this.containerTitle = containerTitle;

        if (hasBehavior(FLAG_HAS_CONTAINER)) {
            this.container = new SimpleContainer(this.slotCount);
        } else {
            this.container = null;
        }
    }

    public GenericRedoxBlockEntity(
            BlockEntityType<?> type,
            BlockPos pos,
            BlockState state,
            String blockEntityId,
            int behaviorFlags
    ) {
        this(type, pos, state, blockEntityId, behaviorFlags, 9, blockEntityId, null);
    }

    public String getBlockEntityId() {
        return this.blockEntityId;
    }

    public int getBehaviorFlags() {
        return this.behaviorFlags;
    }

    public int getSlotCount() {
        return this.slotCount;
    }

    public String getMenuId() {
        return this.menuId;
    }

    public SimpleContainer getContainer() {
        return this.container;
    }

    public boolean hasBehavior(int flag) {
        return (this.behaviorFlags & flag) != 0;
    }

    @Override
    public Component getDisplayName() {
        if (this.containerTitle != null && !this.containerTitle.isEmpty()) {
            return Component.literal(this.containerTitle);
        }
        return Component.literal(this.blockEntityId);
    }

    @Override
    public AbstractContainerMenu createMenu(int containerId, Inventory playerInventory, Player player) {
        MenuType<GenericRedoxMenu> menuType = GenericRedoxMenuHelper.getMenuType(this.menuId);
        if (menuType == null) {
            menuType = GenericRedoxMenuHelper.registerMenuType(this.menuId, this.slotCount);
        }
        return new GenericRedoxMenu(
                menuType,
                containerId,
                playerInventory,
                this.container != null ? this.container : new SimpleContainer(this.slotCount),
                this.menuId,
                this.slotCount
        );
    }

    public void serverTick(Level level, BlockPos pos, BlockState state) {
        if (hasBehavior(FLAG_HAS_TICK)) {
            int x = (pos != null) ? pos.getX() : 0;
            int y = (pos != null) ? pos.getY() : 0;
            int z = (pos != null) ? pos.getZ() : 0;
            RedoxBridge.onBlockEntityTick(this.blockEntityId, x, y, z);
            if (hasBehavior(FLAG_HAS_PERSISTENCE)) {
                setChanged();
            }
        }
    }

    @Override
    protected void saveAdditional(ValueOutput output) {
        super.saveAdditional(output);
        if (hasBehavior(FLAG_HAS_CONTAINER) && this.container != null) {
            ContainerHelper.saveAllItems(output, this.container.getItems());
        }
        if (hasBehavior(FLAG_HAS_PERSISTENCE)) {
            BlockPos pos = getBlockPos();
            int x = (pos != null) ? pos.getX() : 0;
            int y = (pos != null) ? pos.getY() : 0;
            int z = (pos != null) ? pos.getZ() : 0;
            String data = RedoxBridge.onBlockEntitySave(this.blockEntityId, x, y, z);
            if (data != null && !data.isEmpty()) {
                output.putString("redox_data", data);
            }
        }
    }

    @Override
    protected void loadAdditional(ValueInput input) {
        super.loadAdditional(input);
        if (hasBehavior(FLAG_HAS_CONTAINER) && this.container != null) {
            ContainerHelper.loadAllItems(input, this.container.getItems());
        }
        if (hasBehavior(FLAG_HAS_PERSISTENCE)) {
            BlockPos pos = getBlockPos();
            int x = (pos != null) ? pos.getX() : 0;
            int y = (pos != null) ? pos.getY() : 0;
            int z = (pos != null) ? pos.getZ() : 0;
            String data = input.getStringOr("redox_data", "");
            RedoxBridge.onBlockEntityLoad(this.blockEntityId, x, y, z, data);
        }
    }
}
