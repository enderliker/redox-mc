package net.redoxmc.bridge.block;

import net.minecraft.core.BlockPos;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.entity.BlockEntityType;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.storage.ValueInput;
import net.minecraft.world.level.storage.ValueOutput;
import net.redoxmc.bridge.RedoxBridge;

public class GenericRedoxBlockEntity extends BlockEntity {
    public static final int FLAG_HAS_TICK = 1 << 0;
    public static final int FLAG_HAS_PERSISTENCE = 1 << 1;

    private final String blockEntityId;
    private final int behaviorFlags;

    public GenericRedoxBlockEntity(
            BlockEntityType<?> type,
            BlockPos pos,
            BlockState state,
            String blockEntityId,
            int behaviorFlags
    ) {
        super(type, pos, state);
        this.blockEntityId = (blockEntityId != null) ? blockEntityId : "unknown";
        this.behaviorFlags = behaviorFlags;
    }

    public String getBlockEntityId() {
        return this.blockEntityId;
    }

    public int getBehaviorFlags() {
        return this.behaviorFlags;
    }

    public boolean hasBehavior(int flag) {
        return (this.behaviorFlags & flag) != 0;
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
