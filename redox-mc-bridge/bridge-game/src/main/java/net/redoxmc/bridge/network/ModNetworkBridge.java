package net.redoxmc.bridge.network;

import com.mojang.authlib.GameProfile;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.Map;
import java.util.Queue;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.Consumer;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.network.protocol.Packet;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.PacketUtils;
import net.minecraft.network.protocol.common.ClientboundCustomPayloadPacket;
import net.minecraft.network.protocol.common.ServerboundCustomPayloadPacket;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;
import net.minecraft.resources.Identifier;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.RunningOnDifferentThreadException;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.server.network.ConfigurationTask;
import net.minecraft.server.network.ServerCommonPacketListenerImpl;
import net.minecraft.server.network.ServerConfigurationPacketListenerImpl;
import net.redoxmc.bridge.RedoxBridge;
import net.redoxmc.bridge.event.ServerEvents;

/**
 * Bridge hook handler for mod network channels, CustomPacketPayload codecs,
 * outbound packet routing, and the CONFIGURATION handshake phase.
 */
public final class ModNetworkBridge {
    /** Client-side serverbound custom payload limit enforced by vanilla. */
    private static final int MAX_SERVERBOUND_PAYLOAD_SIZE = 32767;

    private static final Map<ServerConfigurationPacketListenerImpl, ModHandshakeConfigurationTask> activeHandshakeTasks =
        new ConcurrentHashMap<>();

    private static final Field SERVER_FIELD = resolveServerField();

    private static volatile Consumer<Object[]> testCaptureSink;

    private static volatile Method clientGetInstance;
    private static volatile Method clientGetConnection;
    private static volatile Method clientSend;

    private ModNetworkBridge() {}

    /**
     * Resolves the protected {@code server} field once so callbacks can schedule
     * themselves onto the server thread without relying on a static singleton.
     */
    private static Field resolveServerField() {
        try {
            Field field = ServerCommonPacketListenerImpl.class.getDeclaredField("server");
            field.setAccessible(true);
            return field;
        } catch (Throwable t) {
            System.err.println("[redox-mc-bridge] Could not access ServerCommonPacketListenerImpl.server: " + t.getMessage());
            return null;
        }
    }

    /**
     * Entry hook executed before CustomPacketPayload.codec() is constructed.
     */
    public static void onPreCustomPayloadCodec() {
        // Kept as a named hook target for symmetry and diagnostics.
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

    /**
     * Routes a handshake response to the task that belongs to the exact connection
     * that received it.
     */
    private static void dispatchHandshakeResponse(
        final ServerConfigurationPacketListenerImpl listener,
        final byte[] data
    ) {
        ModHandshakeConfigurationTask task = activeHandshakeTasks.get(listener);
        if (task == null) {
            System.err.println("[redox-mc-bridge] Received handshake response without an active task; ignoring.");
            return;
        }
        task.handleClientResponse(data);
    }

    /**
     * Handles an inbound serverbound mod payload on the server.
     *
     * The empty vanilla handler runs on the Netty thread, so this bridge
     * schedules the actual dispatch onto the server thread first. The
     * {@link RunningOnDifferentThreadException} must propagate so the
     * connection layer can mark the packet as handled.
     *
     * @param listener the connection that received the payload
     * @param packet the serverbound custom payload packet
     */
    public static void onServerCustomPayload(
        final ServerCommonPacketListenerImpl listener,
        final ServerboundCustomPayloadPacket packet
    ) {
        if (listener == null || packet == null) {
            return;
        }
        if (!(packet.payload() instanceof ModRawPayload modPayload)) {
            return;
        }

        try {
            MinecraftServer server = SERVER_FIELD != null ? (MinecraftServer) SERVER_FIELD.get(listener) : null;
            if (server == null) {
                System.err.println("[redox-mc-bridge] Dropping inbound mod payload: unavailable MinecraftServer.");
                return;
            }

            PacketUtils.ensureRunningOnSameThread(packet, listener, server.packetProcessor());

            if (listener instanceof ServerConfigurationPacketListenerImpl configListener
                    && modPayload.id().equals(ModHandshakeConfigurationTask.HANDSHAKE_RESPONSE_CHANNEL)) {
                dispatchHandshakeResponse(configListener, modPayload.data());
                return;
            }

            GameProfile owner = listener.getOwner();
            String senderName = owner != null ? owner.name() : "unknown";
            String senderUuid = (owner != null && owner.id() != null) ? owner.id().toString() : "";

            RedoxBridge.onModCustomPayloadFrom(
                modPayload.id().toString(),
                modPayload.data(),
                senderName,
                senderUuid,
                RedoxBridge.NETWORK_SIDE_SERVER
            );
        } catch (RunningOnDifferentThreadException runningOnDifferentThread) {
            throw runningOnDifferentThread;
        } catch (Throwable t) {
            System.err.println("[redox-mc-bridge] Error dispatching inbound mod payload: " + t.getMessage());
        }
    }

    /**
     * Handles an inbound clientbound mod payload on the client.
     *
     * Vanilla already rescheduled the packet onto the client thread before this
     * method is invoked, so the native dispatch and outbox flush run on the
     * correct thread.
     */
    public static void onClientCustomPayload(final CustomPacketPayload payload) {
        if (!(payload instanceof ModRawPayload modPayload)) {
            return;
        }

        try {
            RedoxBridge.onModCustomPayloadFrom(
                modPayload.id().toString(),
                modPayload.data(),
                "server",
                "",
                RedoxBridge.NETWORK_SIDE_CLIENT
            );
        } catch (Throwable t) {
            System.err.println("[redox-mc-bridge] Error dispatching clientbound mod payload: " + t.getMessage());
        }
    }

    /**
     * Flushes queued outbound mod messages on the client thread.
     */
    public static void onClientTick() {
        try {
            RedoxBridge.onClientTick();
        } catch (Throwable t) {
            System.err.println("[redox-mc-bridge] Error flushing mod network outbox on client tick: " + t.getMessage());
        }
    }

    /**
     * Sends a queued outbound mod message.
     *
     * @param channel full channel identifier (namespace:path)
     * @param targetKind one of {@link RedoxBridge#NETWORK_TARGET_BROADCAST},
     *                   {@link RedoxBridge#NETWORK_TARGET_PLAYER}, or
     *                   {@link RedoxBridge#NETWORK_TARGET_SERVER}
     * @param targetName target player name for player-targeted messages
     * @param targetUuid target player UUID for player-targeted messages
     * @param data payload bytes
     * @param originSide side on which the sending mod callback executed
     * @return true when the message was accepted by the transport
     */
    public static boolean sendModPayload(
        final String channel,
        final int targetKind,
        final String targetName,
        final String targetUuid,
        final byte[] data,
        final int originSide
    ) {
        if (channel == null || data == null) {
            return false;
        }

        Consumer<Object[]> sink = testCaptureSink;
        if (sink != null) {
            sink.accept(new Object[] { channel, targetKind, targetName, targetUuid, data, originSide });
            return true;
        }

        Identifier identifier;
        try {
            int separator = channel.indexOf(':');
            if (separator <= 0 || separator == channel.length() - 1) {
                return false;
            }
            identifier = Identifier.fromNamespaceAndPath(channel.substring(0, separator), channel.substring(separator + 1));
        } catch (Throwable t) {
            System.err.println("[redox-mc-bridge] Invalid outbound mod channel '" + channel + "': " + t.getMessage());
            return false;
        }

        if (originSide == RedoxBridge.NETWORK_SIDE_SERVER) {
            if (data.length > ModRawPayload.MAX_PAYLOAD_SIZE) {
                System.err.println("[redox-mc-bridge] Outbound clientbound payload exceeds 1 MiB on channel '" + channel + "'.");
                return false;
            }
            return sendFromServer(identifier, data, targetKind, targetName, targetUuid);
        }

        if (originSide == RedoxBridge.NETWORK_SIDE_CLIENT) {
            if (data.length > MAX_SERVERBOUND_PAYLOAD_SIZE) {
                System.err.println("[redox-mc-bridge] Outbound serverbound payload exceeds 32767 bytes on channel '" + channel + "'.");
                return false;
            }
            return sendFromClient(identifier, data, targetKind);
        }

        return false;
    }

    private static boolean sendFromServer(
        final Identifier identifier,
        final byte[] data,
        final int targetKind,
        final String targetName,
        final String targetUuid
    ) {
        MinecraftServer server = ServerEvents.getServer();
        if (server == null) {
            System.err.println("[redox-mc-bridge] Cannot send clientbound mod payload: MinecraftServer is not captured yet.");
            return false;
        }

        ModRawPayload payload = new ModRawPayload(identifier, data);
        switch (targetKind) {
            case RedoxBridge.NETWORK_TARGET_BROADCAST:
                for (ServerPlayer player : server.getPlayerList().getPlayers()) {
                    player.connection.send(new ClientboundCustomPayloadPacket(payload));
                }
                return true;
            case RedoxBridge.NETWORK_TARGET_PLAYER:
                ServerPlayer target = findPlayer(server, targetName, targetUuid);
                if (target == null) {
                    return false;
                }
                target.connection.send(new ClientboundCustomPayloadPacket(payload));
                return true;
            default:
                return false;
        }
    }

    private static ServerPlayer findPlayer(
        final MinecraftServer server,
        final String targetName,
        final String targetUuid
    ) {
        if (targetUuid != null && !targetUuid.isEmpty()) {
            try {
                ServerPlayer byUuid = server.getPlayerList().getPlayer(UUID.fromString(targetUuid));
                if (byUuid != null) {
                    return byUuid;
                }
            } catch (IllegalArgumentException ignored) {
                // Fall through to name lookup.
            }
        }
        if (targetName != null && !targetName.isEmpty()) {
            return server.getPlayerList().getPlayerByName(targetName);
        }
        return null;
    }

    private static boolean sendFromClient(
        final Identifier identifier,
        final byte[] data,
        final int targetKind
    ) {
        if (targetKind != RedoxBridge.NETWORK_TARGET_BROADCAST && targetKind != RedoxBridge.NETWORK_TARGET_SERVER) {
            return false;
        }
        if (!ensureClientReflection()) {
            return false;
        }

        try {
            Object minecraft = clientGetInstance.invoke(null);
            if (minecraft == null) {
                return false;
            }
            Object connection = clientGetConnection.invoke(minecraft);
            if (connection == null) {
                return false;
            }
            clientSend.invoke(connection, new ServerboundCustomPayloadPacket(new ModRawPayload(identifier, data)));
            return true;
        } catch (Throwable t) {
            System.err.println("[redox-mc-bridge] Failed to send serverbound mod payload: " + t.getMessage());
            return false;
        }
    }

    private static synchronized boolean ensureClientReflection() {
        if (clientSend != null) {
            return true;
        }
        try {
            Class<?> minecraftClass = Class.forName("net.minecraft.client.Minecraft");
            Class<?> listenerClass = Class.forName("net.minecraft.client.multiplayer.ClientCommonPacketListenerImpl");
            clientGetInstance = minecraftClass.getMethod("getInstance");
            clientGetConnection = minecraftClass.getMethod("getConnection");
            clientSend = listenerClass.getMethod("send", Packet.class);
            return true;
        } catch (Throwable t) {
            System.err.println("[redox-mc-bridge] Client network bridge unavailable: " + t.getMessage());
            return false;
        }
    }

    /**
     * Installs a capture sink for integration tests. When non-null, outbound
     * messages are delivered to the sink instead of the real transport.
     */
    public static void setTestCaptureSink(final Consumer<Object[]> sink) {
        testCaptureSink = sink;
    }

    /**
     * Removes the test capture sink, restoring the real transport.
     */
    public static void clearTestCaptureSink() {
        testCaptureSink = null;
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

                // Mod namespace: retain complete binary buffer. Native dispatch now
                // happens from the listener hooks, where sender identity is available.
                int length = input.readableBytes();
                if (length > ModRawPayload.MAX_PAYLOAD_SIZE) {
                    throw new IllegalArgumentException(
                        "Mod payload for channel '" + identifier + "' exceeds limit (" + length + " > " + ModRawPayload.MAX_PAYLOAD_SIZE + ")"
                    );
                }

                byte[] data = new byte[length];
                input.readBytes(data);
                return new ModRawPayload(identifier, data);
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
