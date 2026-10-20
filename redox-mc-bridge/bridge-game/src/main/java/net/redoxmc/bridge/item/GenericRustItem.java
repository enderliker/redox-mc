package net.redoxmc.bridge.item;

import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.context.UseOnContext;
import net.minecraft.world.level.Level;
import net.redoxmc.bridge.RedoxBridge;

/**
 * Universal Rust mod item bridge extending vanilla net.minecraft.world.item.Item.
 *
 * Resides in bridge-game and is defined into the game's ClassLoader.
 * Delegates interactions (use, useOn, finishUsingItem) to native callbacks
 * in RedoxBridge ONLY if the mod explicitly declared active behavior for this item.
 * If the item is passive, all JNI crossings are completely bypassed.
 */
public class GenericRustItem extends Item {
    public static final int FLAG_HAS_USE = 1 << 0;
    public static final int FLAG_HAS_USE_ON = 1 << 1;
    public static final int FLAG_HAS_FINISH_USING = 1 << 2;

    private final String itemId;
    private final int behaviorFlags;

    /**
     * Constructs a new GenericRustItem.
     *
     * @param properties Vanilla Item.Properties (including max stack size, durability, rarity, etc.)
     * @param itemId Namespaced identifier of the item (e.g. "testmod:ruby")
     * @param behaviorFlags Bitmask of active behaviors (FLAG_HAS_USE, FLAG_HAS_USE_ON, FLAG_HAS_FINISH_USING)
     */
    public GenericRustItem(Item.Properties properties, String itemId, int behaviorFlags) {
        super(properties);
        this.itemId = (itemId != null) ? itemId : "unknown";
        this.behaviorFlags = behaviorFlags;
    }

    /**
     * Constructs a passive GenericRustItem with no active interaction callbacks.
     */
    public GenericRustItem(Item.Properties properties, String itemId) {
        this(properties, itemId, 0);
    }

    public String getItemId() {
        return this.itemId;
    }

    public int getBehaviorFlags() {
        return this.behaviorFlags;
    }

    public boolean hasBehavior(int flag) {
        return (this.behaviorFlags & flag) != 0;
    }

    @Override
    public InteractionResult use(Level level, Player player, InteractionHand hand) {
        if (!hasBehavior(FLAG_HAS_USE)) {
            return super.use(level, player, hand);
        }
        int handId = (hand == InteractionHand.MAIN_HAND) ? 0 : 1;
        int res = RedoxBridge.onItemUse(this.itemId, handId);
        return mapInteractionResult(res, super.use(level, player, hand));
    }

    @Override
    public InteractionResult useOn(UseOnContext context) {
        if (!hasBehavior(FLAG_HAS_USE_ON)) {
            return super.useOn(context);
        }
        if (context == null) {
            return super.useOn(context);
        }
        BlockPos pos = context.getClickedPos();
        Direction face = context.getClickedFace();
        int x = (pos != null) ? pos.getX() : 0;
        int y = (pos != null) ? pos.getY() : 0;
        int z = (pos != null) ? pos.getZ() : 0;
        int faceId = (face != null) ? face.get3DDataValue() : 0;
        int handId = (context.getHand() == InteractionHand.MAIN_HAND) ? 0 : 1;

        int res = RedoxBridge.onItemUseOn(this.itemId, x, y, z, faceId, handId);
        return mapInteractionResult(res, super.useOn(context));
    }

    @Override
    public ItemStack finishUsingItem(ItemStack stack, Level level, LivingEntity entity) {
        if (hasBehavior(FLAG_HAS_FINISH_USING)) {
            RedoxBridge.onItemFinishUsing(this.itemId);
        }
        return super.finishUsingItem(stack, level, entity);
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
     * Test verification helper to confirm instantiation, properties, and behavior flags
     * in the runtime Minecraft classloader.
     */
    public static String verifyItemInstantiation() {
        Item.Properties props = new Item.Properties();
        GenericRustItem item = new GenericRustItem(props, "testmod:ruby", FLAG_HAS_USE);
        if (!"testmod:ruby".equals(item.getItemId())) {
            throw new IllegalStateException("Unexpected itemId: " + item.getItemId());
        }
        if (!item.hasBehavior(FLAG_HAS_USE) || item.hasBehavior(FLAG_HAS_USE_ON)) {
            throw new IllegalStateException("Unexpected behavior flags: " + item.getBehaviorFlags());
        }
        return "SUCCESS: GenericRustItem instantiated: " + item.getItemId()
                + " (flags=" + item.getBehaviorFlags()
                + ", superclass=" + item.getClass().getSuperclass().getName() + ")";
    }
}
