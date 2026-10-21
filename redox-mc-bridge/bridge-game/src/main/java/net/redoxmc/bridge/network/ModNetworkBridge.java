package net.redoxmc.bridge.network;

import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;
import net.minecraft.resources.Identifier;
import net.redoxmc.bridge.RedoxBridge;

/**
 * Bridge hook handler for CustomPacketPayload.codec().
 *
 * Intercepts CustomPacketPayload encoding and decoding so that mod channels
 * decode into ModRawPayload and are dispatched to the native Rust loader,
 * while vanilla payloads (e.g. BrandPayload) remain completely unaltered.
 */
public final class ModNetworkBridge {
    private ModNetworkBridge() {}

    /**
     * Entry hook executed before CustomPacketPayload.codec() is constructed.
     */
    public static void onPreCustomPayloadCodec() {
        // Pre-hook entrypoint
    }

    /**
     * Post-hook executed on the StreamCodec returned by CustomPacketPayload.codec$original().
     * Wraps the codec to intercept mod namespaces while delegating vanilla namespaces.
     */
    @SuppressWarnings("unchecked")
    public static <B extends FriendlyByteBuf> StreamCodec<B, CustomPacketPayload> wrapCodec(
        final StreamCodec<B, CustomPacketPayload> originalCodec
    ) {
        return new StreamCodec<B, CustomPacketPayload>() {
            @Override
            public CustomPacketPayload decode(final B input) {
                int startIdx = input.readerIndex();
                Identifier identifier = input.readIdentifier();
                String ns = identifier.getNamespace();

                // If namespace is vanilla ("minecraft" or "realms"), rewind and let vanilla decode
                if (ns.equals("minecraft") || ns.equals("realms")) {
                    input.readerIndex(startIdx);
                    return originalCodec.decode(input);
                }

                // Mod namespace: retain complete binary buffer and forward to native Rust
                int length = input.readableBytes();
                if (length > ModRawPayload.MAX_PAYLOAD_SIZE) {
                    throw new IllegalArgumentException(
                        "Mod payload for channel '" + identifier + "' exceeds limit (" + length + " > " + ModRawPayload.MAX_PAYLOAD_SIZE + ")"
                    );
                }

                byte[] data = new byte[length];
                input.readBytes(data);
                ModRawPayload payload = new ModRawPayload(identifier, data);

                // Dispatch to Rust via JNI
                try {
                    RedoxBridge.onModCustomPayload(ns, identifier.getPath(), data);
                } catch (Throwable t) {
                    System.err.println("[redox-mc-bridge] Error in native mod payload dispatch: " + t.getMessage());
                }

                return payload;
            }

            @Override
            public void encode(final B output, final CustomPacketPayload value) {
                if (value instanceof ModRawPayload modPayload) {
                    output.writeIdentifier(modPayload.id());
                    output.writeBytes(modPayload.data());
                } else {
                    originalCodec.encode(output, value);
                }
            }
        };
    }
}
