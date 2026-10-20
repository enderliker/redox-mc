package net.redoxmc.bridge.block;

import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.Registry;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.Item;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.LevelAccessor;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockBehaviour;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.BlockHitResult;
import net.redoxmc.bridge.RedoxBridge;

/**
 * Universal Rust mod block bridge extending vanilla net.minecraft.world.level.block.Block.
 *
 * Resides in bridge-game and is defined into the game's ClassLoader.
 * Delegates block interactions (useWithoutItem, stepOn, destroy) to native callbacks
 * in RedoxBridge ONLY if the mod explicitly declared active behavior for this block.
 * If the block is passive, all JNI crossings are completely bypassed.
 */
public class GenericRedoxBlock extends Block {
    public static final int FLAG_HAS_USE_WITHOUT_ITEM = 1 << 0;
    public static final int FLAG_HAS_STEP_ON = 1 << 1;
    public static final int FLAG_HAS_DESTROY = 1 << 2;

    private final String blockId;
    private final int behaviorFlags;

    /**
     * Constructs a new GenericRedoxBlock.
     *
     * @param properties Vanilla BlockBehaviour.Properties (hardness, resistance, etc.)
     * @param blockId Namespaced identifier of the block (e.g. "testmod:ruby_block")
     * @param behaviorFlags Bitmask of active behaviors (FLAG_HAS_USE_WITHOUT_ITEM, FLAG_HAS_STEP_ON, FLAG_HAS_DESTROY)
     */
    public GenericRedoxBlock(BlockBehaviour.Properties properties, String blockId, int behaviorFlags) {
        super(properties);
        this.blockId = (blockId != null) ? blockId : "unknown";
        this.behaviorFlags = behaviorFlags;
    }

    /**
     * Constructs a passive GenericRedoxBlock with no active interaction callbacks.
     */
    public GenericRedoxBlock(BlockBehaviour.Properties properties, String blockId) {
        this(properties, blockId, 0);
    }

    public String getBlockId() {
        return this.blockId;
    }

    public int getBehaviorFlags() {
        return this.behaviorFlags;
    }

    public boolean hasBehavior(int flag) {
        return (this.behaviorFlags & flag) != 0;
    }

    @Override
    protected InteractionResult useWithoutItem(
            BlockState state, Level level, BlockPos pos, Player player, BlockHitResult hitResult
    ) {
        if (!hasBehavior(FLAG_HAS_USE_WITHOUT_ITEM)) {
            return super.useWithoutItem(state, level, pos, player, hitResult);
        }
        int x = (pos != null) ? pos.getX() : 0;
        int y = (pos != null) ? pos.getY() : 0;
        int z = (pos != null) ? pos.getZ() : 0;
        Direction face = (hitResult != null) ? hitResult.getDirection() : null;
        int faceId = (face != null) ? face.get3DDataValue() : 0;

        int res = RedoxBridge.onBlockUseWithoutItem(this.blockId, x, y, z, faceId);
        return mapInteractionResult(res, super.useWithoutItem(state, level, pos, player, hitResult));
    }

    @Override
    public void stepOn(Level level, BlockPos pos, BlockState onState, Entity entity) {
        if (hasBehavior(FLAG_HAS_STEP_ON)) {
            int x = (pos != null) ? pos.getX() : 0;
            int y = (pos != null) ? pos.getY() : 0;
            int z = (pos != null) ? pos.getZ() : 0;
            RedoxBridge.onBlockStepOn(this.blockId, x, y, z);
        }
        super.stepOn(level, pos, onState, entity);
    }

    @Override
    public void destroy(LevelAccessor level, BlockPos pos, BlockState state) {
        if (hasBehavior(FLAG_HAS_DESTROY)) {
            int x = (pos != null) ? pos.getX() : 0;
            int y = (pos != null) ? pos.getY() : 0;
            int z = (pos != null) ? pos.getZ() : 0;
            RedoxBridge.onBlockDestroy(this.blockId, x, y, z);
        }
        super.destroy(level, pos, state);
    }

    private static InteractionResult mapInteractionResult(int res, InteractionResult fallback) {
        switch (res) {
            case 1:
                return InteractionResult.SUCCESS;
            case 2:
                return InteractionResult.CONSUME;
            case 3:
                return InteractionResult.FAIL;
            case 0:
            default:
                return fallback != null ? fallback : InteractionResult.PASS;
        }
    }

    /**
     * Factory and registration helper to register a GenericRedoxBlock into BuiltInRegistries.BLOCK,
     * add its states into Block.BLOCK_STATE_REGISTRY, and register its associated BlockItem into BuiltInRegistries.ITEM.
     * Must be called while BuiltInRegistries is open (e.g. during onRegisterContent).
     */
    public static GenericRedoxBlock register(
            String blockId,
            float destroyTime,
            float explosionResistance,
            boolean requiresTool,
            boolean noCollision,
            int behaviorFlags,
            boolean registerItem
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

        // 1. Instantiate and register block
        GenericRedoxBlock block = new GenericRedoxBlock(props, blockId, behaviorFlags);
        Registry.register(BuiltInRegistries.BLOCK, blockKey, block);

        // 2. Register every BlockState in Block.BLOCK_STATE_REGISTRY and init cache
        for (BlockState state : block.getStateDefinition().getPossibleStates()) {
            Block.BLOCK_STATE_REGISTRY.add(state);
            state.initCache();
        }

        // 3. Register associated BlockItem into BuiltInRegistries.ITEM and link in Item.BY_BLOCK
        if (registerItem) {
            ResourceKey<Item> itemKey = ResourceKey.create(Registries.ITEM, Identifier.parse(blockId));
            Item.Properties itemProps = new Item.Properties().setId(itemKey).useBlockDescriptionPrefix();
            BlockItem blockItem = new BlockItem(block, itemProps);
            blockItem.registerBlocks(Item.BY_BLOCK, blockItem);
            Registry.register(BuiltInRegistries.ITEM, itemKey, blockItem);
        }

        return block;
    }

    /**
     * Test verification helper to confirm instantiation and behavior flags in runtime ClassLoader.
     */
    public static String verifyBlockInstantiation() {
        ResourceKey<Block> testKey = ResourceKey.create(Registries.BLOCK, Identifier.parse("testmod:ruby_block"));
        BlockBehaviour.Properties props = BlockBehaviour.Properties.of().setId(testKey);
        GenericRedoxBlock block = new GenericRedoxBlock(props, "testmod:ruby_block", FLAG_HAS_USE_WITHOUT_ITEM);
        if (!"testmod:ruby_block".equals(block.getBlockId())) {
            throw new IllegalStateException("Unexpected blockId: " + block.getBlockId());
        }
        if (!block.hasBehavior(FLAG_HAS_USE_WITHOUT_ITEM) || block.hasBehavior(FLAG_HAS_STEP_ON)) {
            throw new IllegalStateException("Unexpected behavior flags: " + block.getBehaviorFlags());
        }
        return "SUCCESS: GenericRedoxBlock instantiated: " + block.getBlockId()
                + " (flags=" + block.getBehaviorFlags()
                + ", superclass=" + block.getClass().getSuperclass().getName() + ")";
    }

    /**
     * Test verification helper to inspect registered block state ID and BlockItem association.
     */
    public static String verifyBlockRegistration(String blockId) {
        Identifier id = Identifier.parse(blockId);
        Block block = BuiltInRegistries.BLOCK.getValue(id);
        if (!(block instanceof GenericRedoxBlock redoxBlock)) {
            return "ERROR: Block '" + blockId + "' is not registered or not a GenericRedoxBlock";
        }
        BlockState defaultState = redoxBlock.defaultBlockState();
        int stateId = Block.getId(defaultState);
        if (stateId <= 0) {
            return "ERROR: BlockState for '" + blockId + "' has invalid state ID: " + stateId;
        }
        Item item = Item.BY_BLOCK.get(redoxBlock);
        if (!(item instanceof BlockItem)) {
            return "ERROR: Item.BY_BLOCK does not map to a BlockItem for '" + blockId + "'";
        }
        Item registeredItem = BuiltInRegistries.ITEM.getValue(id);
        if (registeredItem != item) {
            return "ERROR: BuiltInRegistries.ITEM does not match Item.BY_BLOCK for '" + blockId + "'";
        }
        return "SUCCESS: GenericRedoxBlock '" + blockId + "' registered with state ID " + stateId
                + ", BlockItem: " + item.getClass().getName();
    }
}
