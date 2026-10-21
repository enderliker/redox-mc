package net.redoxmc.bridge.network;

import java.lang.reflect.Field;
import java.util.Map;
import java.util.Queue;
import java.util.concurrent.ConcurrentHashMap;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;
import net.minecraft.resources.Identifier;
import net.minecraft.server.network.ConfigurationTask;
import net.minecraft.server.network.ServerConfigurationPacketListenerImpl;
import net.redoxmc.bridge.RedoxBridge;

/**
 * Bridge hook handler for network operations, CustomPacketPayload.codec(),
 * and configuration handshake phase.
 */
public final class ModNetworkBridge {
    private static final Map<ServerConfigurationPacketListenerImpl, ModHandshakeConfigurationTask> activeHandshakeTasks =
        new ConcurrentHashMap<>();

    private ModNetworkBridge() {}

    /**
     * Entry hook executed before CustomPacketPayload.codec() is constructed.
     */
    public static void onPreCustomPayloadCodec() {
        // Pre-hook entrypoint
    }

    /**
     * Entry hook executed at the beginning of ServerConfigurationPacketListenerImpl.startConfiguration().
     * Enqueues ModHandshakeConfigurationTask into the listener's configurationTasks queue.
     */
    public static void onStartConfiguration(final ServerConfigurationPacketListenerImpl listener) {
        try {
            Field field = ServerConfigurationPacketListenerImpl.class.getDeclaredField("configurationTasks");
            field.setAccessible(true);
            @SuppressWarnings("unchecked")
            Queue<ConfigurationTask> tasks = (Queue<ConfigurationTask>) field.get(listener);
            if (tasks != null) {
                ModHandshakeConfigurationTask task = new ModHandshakeConfigurationTask(listener);
                tasks.add(task);
                System.out.println("[redox-mc-bridge] Enqueued ModHandshakeConfigurationTask in CONFIGURATION phase.");
            }
        } catch (Throwable t) {
            System.err.println("[redox-mc-bridge] Failed to enqueue ModHandshakeConfigurationTask: " + t.getMessage());
        }
    }

    public static void registerActiveHandshakeTask(
        final ServerConfigurationPacketListenerImpl listener,
        final ModHandshakeConfigurationTask task
    ) {
        activeHandshakeTasks.put(listener, task);
    }

    public static void unregisterActiveHandshakeTask(final ServerConfigurationPacketListenerImpl listener) {
        activeHandshakeTasks.remove(listener);
    }

    public static void dispatchHandshakeResponse(final byte[] data) {
        for (ModHandshakeConfigurationTask task : activeHandshakeTasks.values()) {
            task.handleClientResponse(data);
            break;
        }
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

                // If this is a handshake response, dispatch to active handshake task
                if (ns.equals("redox") && identifier.getPath().equals("handshake_response")) {
                    dispatchHandshakeResponse(data);
                }

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
