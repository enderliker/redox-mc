package net.redoxmc.bridge.block;

import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Registry;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.Item;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.EntityBlock;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.entity.BlockEntityTicker;
import net.minecraft.world.level.block.entity.BlockEntityType;
import net.minecraft.world.level.block.state.BlockBehaviour;
import net.minecraft.world.level.block.state.BlockState;

public class GenericRedoxEntityBlock extends GenericRedoxBlock implements EntityBlock {
    private final String blockEntityId;
    private final int entityBehaviorFlags;
    private BlockEntityType<GenericRedoxBlockEntity> blockEntityType;

    public GenericRedoxEntityBlock(
            BlockBehaviour.Properties properties,
            String blockId,
            int blockBehaviorFlags,
            String blockEntityId,
            int entityBehaviorFlags
    ) {
        super(properties, blockId, blockBehaviorFlags);
        this.blockEntityId = blockEntityId;
        this.entityBehaviorFlags = entityBehaviorFlags;
    }

    public void setBlockEntityType(BlockEntityType<GenericRedoxBlockEntity> type) {
        this.blockEntityType = type;
    }

    public BlockEntityType<GenericRedoxBlockEntity> getBlockEntityType() {
        return this.blockEntityType;
    }

    public String getBlockEntityId() {
        return this.blockEntityId;
    }

    public int getEntityBehaviorFlags() {
        return this.entityBehaviorFlags;
    }

    @Override
    public BlockEntity newBlockEntity(BlockPos pos, BlockState state) {
        return new GenericRedoxBlockEntity(
                this.blockEntityType, pos, state, this.blockEntityId, this.entityBehaviorFlags
        );
    }

    @Override
    @SuppressWarnings("unchecked")
    public <T extends BlockEntity> BlockEntityTicker<T> getTicker(
            Level level, BlockState state, BlockEntityType<T> type
    ) {
        if (level.isClientSide() || (this.entityBehaviorFlags & GenericRedoxBlockEntity.FLAG_HAS_TICK) == 0) {
            return null;
        }
        return (lvl, pos, st, be) -> {
            if (be instanceof GenericRedoxBlockEntity grbe) {
                grbe.serverTick(lvl, pos, st);
            }
        };
    }

    public static GenericRedoxEntityBlock registerWithEntity(
            String blockId,
            float destroyTime,
            float explosionResistance,
            boolean requiresTool,
            boolean noCollision,
            int blockBehaviorFlags,
            boolean registerItem,
            String blockEntityId,
            int entityBehaviorFlags
    ) {
        ResourceKey<Block> blockKey = ResourceKey.create(Registries.BLOCK, Identifier.parse(blockId));
        BlockBehaviour.Properties props = BlockBehaviour.Properties.of().setId(blockKey);
        if (destroyTime >= 0.0f || explosionResistance >= 0.0f) {
            props.strength(Math.max(0.0f, destroyTime), Math.max(0.0f, explosionResistance));
        }
        if (requiresTool) {
            props.requiresCorrectToolForDrops();
        }
        if (noCollision) {
            props.noCollision();
        }

        GenericRedoxEntityBlock block = new GenericRedoxEntityBlock(
                props, blockId, blockBehaviorFlags, blockEntityId, entityBehaviorFlags
        );

        Registry.register(BuiltInRegistries.BLOCK, blockKey, block);
        for (BlockState state : block.getStateDefinition().getPossibleStates()) {
            Block.BLOCK_STATE_REGISTRY.add(state);
            state.initCache();
        }

        if (registerItem) {
            ResourceKey<Item> itemKey = ResourceKey.create(Registries.ITEM, Identifier.parse(blockId));
            Item.Properties itemProps =
                    new Item.Properties().useBlockDescriptionPrefix().setId(itemKey);
            BlockItem blockItem = new BlockItem(block, itemProps);
            blockItem.registerBlocks(Item.BY_BLOCK, blockItem);
            Registry.register(BuiltInRegistries.ITEM, itemKey, blockItem);
        }

        BlockEntityType<GenericRedoxBlockEntity> beType = new BlockEntityType<>(
                (pos, state) -> new GenericRedoxBlockEntity(
                        block.getBlockEntityType(), pos, state, blockEntityId, entityBehaviorFlags
                ),
                Set.of(block)
        );
        block.setBlockEntityType(beType);
        Registry.register(BuiltInRegistries.BLOCK_ENTITY_TYPE, Identifier.parse(blockEntityId), beType);

        return block;
    }

    public static BlockEntityType<GenericRedoxBlockEntity> registerBlockEntityType(
            String blockEntityId,
            String[] validBlockIds,
            int entityBehaviorFlags
    ) {
        Set<Block> blocks = new java.util.HashSet<>();
        if (validBlockIds != null) {
            for (String blockId : validBlockIds) {
                Block b = BuiltInRegistries.BLOCK.getValue(Identifier.parse(blockId));
                if (b != null) {
                    blocks.add(b);
                }
            }
        }
        BlockEntityType<GenericRedoxBlockEntity> beType = new BlockEntityType<>(
                (pos, state) -> new GenericRedoxBlockEntity(
                        BuiltInRegistries.BLOCK_ENTITY_TYPE.getValue(Identifier.parse(blockEntityId)),
                        pos, state, blockEntityId, entityBehaviorFlags
                ),
                blocks
        );
        Registry.register(BuiltInRegistries.BLOCK_ENTITY_TYPE, Identifier.parse(blockEntityId), beType);
        return beType;
    }
}

