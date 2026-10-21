package net.redoxmc.bridge.network;

import java.util.Arrays;
import java.util.Objects;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;
import net.minecraft.resources.Identifier;

/**
 * Universal Mod Network CustomPacketPayload representation.
 *
 * Preserves complete raw byte buffer for mod channel payloads, preventing
 * them from falling into vanilla DiscardedPayload.
 */
public record ModRawPayload(Identifier id, byte[] data) implements CustomPacketPayload {
    public static final int MAX_PAYLOAD_SIZE = 1048576; // 1 MB payload limit for mod channels

    public ModRawPayload {
        Objects.requireNonNull(id, "id must not be null");
        Objects.requireNonNull(data, "data must not be null");
    }

    @Override
    public Type<ModRawPayload> type() {
        return new Type<>(this.id);
    }

    public static <B extends FriendlyByteBuf> StreamCodec<B, ModRawPayload> codec(final Identifier id) {
        return codec(id, MAX_PAYLOAD_SIZE);
    }

    public static <B extends FriendlyByteBuf> StreamCodec<B, ModRawPayload> codec(final Identifier id, final int maxPayloadSize) {
        return CustomPacketPayload.codec(
            (payload, buf) -> {
                buf.writeBytes(payload.data());
            },
            buf -> {
                int length = buf.readableBytes();
                if (length >= 0 && length <= maxPayloadSize) {
                    byte[] bytes = new byte[length];
                    buf.readBytes(bytes);
                    return new ModRawPayload(id, bytes);
                } else {
                    throw new IllegalArgumentException(
                        "Payload size " + length + " exceeds maximum allowed: " + maxPayloadSize
                    );
                }
            }
        );
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof ModRawPayload that)) return false;
        return Objects.equals(this.id, that.id) && Arrays.equals(this.data, that.data);
    }

    @Override
    public int hashCode() {
        int result = Objects.hash(this.id);
        result = 31 * result + Arrays.hashCode(this.data);
        return result;
    }

    @Override
    public String toString() {
        return "ModRawPayload[id=" + this.id + ", bytes=" + this.data.length + "]";
    }
}
