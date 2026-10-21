package net.redoxmc.bridge.network;

import io.netty.buffer.Unpooled;
import java.util.function.Consumer;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.network.chat.Component;
import net.minecraft.network.protocol.Packet;
import net.minecraft.network.protocol.common.ClientboundCustomPayloadPacket;
import net.minecraft.resources.Identifier;
import net.minecraft.server.network.ConfigurationTask;
import net.minecraft.server.network.ServerConfigurationPacketListenerImpl;
import net.redoxmc.bridge.RedoxBridge;

/**
 * Configuration task executed in CONFIGURATION phase before player enters PLAY phase.
 *
 * Exchanges registry content hashes and mod manifests between server and client.
 * If client differs in registry content hash or required mods, it is disconnected
 * with a descriptive error message.
 */
public class ModHandshakeConfigurationTask implements ConfigurationTask {
    public static final ConfigurationTask.Type TYPE = new ConfigurationTask.Type("redox:mod_handshake");
    public static final Identifier HANDSHAKE_REQUEST_CHANNEL = Identifier.fromNamespaceAndPath("redox", "handshake_request");
    public static final Identifier HANDSHAKE_RESPONSE_CHANNEL = Identifier.fromNamespaceAndPath("redox", "handshake_response");

    private final ServerConfigurationPacketListenerImpl listener;
    private boolean finished = false;
    private int ticksWaiting = 0;
    private static final int TIMEOUT_TICKS = 20 * 15; // 15 seconds

    public ModHandshakeConfigurationTask(final ServerConfigurationPacketListenerImpl listener) {
        this.listener = listener;
    }

    @Override
    public void start(final Consumer<Packet<?>> connection) {
        String serverHash = RedoxBridge.getRegistryHash();
        String[] modIds = RedoxBridge.getLoadedModIds();
        String[] modVersions = RedoxBridge.getLoadedModVersions();

        // If server has no custom registry content and no mods, handshake completes immediately
        if ((serverHash == null || serverHash.isEmpty()) && (modIds == null || modIds.length == 0)) {
            System.out.println("[redox-mc-bridge] No custom mods or registry content on server; skipping handshake.");
            this.finished = true;
            return;
        }

        // Register as active handshake task for this listener
        ModNetworkBridge.registerActiveHandshakeTask(this.listener, this);

        // Serialize handshake request
        FriendlyByteBuf buf = new FriendlyByteBuf(Unpooled.buffer());
        buf.writeUtf(serverHash != null ? serverHash : "");
        int modCount = modIds != null ? modIds.length : 0;
        buf.writeVarInt(modCount);
        for (int i = 0; i < modCount; i++) {
            buf.writeUtf(modIds[i]);
            buf.writeUtf(modVersions[i]);
        }

        byte[] payloadBytes = new byte[buf.readableBytes()];
        buf.readBytes(payloadBytes);

        System.out.println("[redox-mc-bridge] Sending mod handshake request to client (hash=" + serverHash + ", mods=" + modCount + ")...");
        connection.accept(new ClientboundCustomPayloadPacket(new ModRawPayload(HANDSHAKE_REQUEST_CHANNEL, payloadBytes)));
    }

    @Override
    public boolean tick() {
        if (this.finished) {
            return true;
        }

        this.ticksWaiting++;
        if (this.ticksWaiting > TIMEOUT_TICKS) {
            System.err.println("[redox-mc-bridge] Mod handshake timed out waiting for client response.");
            this.listener.disconnect(
                Component.literal("Mod handshake timed out waiting for client response. Ensure Redox-MC is installed on the client.")
            );
            return false;
        }

        return false;
    }

    public void handleClientResponse(final byte[] data) {
        try {
            FriendlyByteBuf buf = new FriendlyByteBuf(Unpooled.wrappedBuffer(data));
            String clientHash = buf.readUtf();
            int modCount = buf.readVarInt();
            String[] clientModIds = new String[modCount];
            String[] clientModVersions = new String[modCount];
            for (int i = 0; i < modCount; i++) {
                clientModIds[i] = buf.readUtf();
                clientModVersions[i] = buf.readUtf();
            }

            // Validate against server in Rust via JNI
            String errorReason = RedoxBridge.validateClientHandshake(clientHash, clientModIds, clientModVersions);
            if (errorReason != null && !errorReason.isEmpty()) {
                System.err.println("[redox-mc-bridge] Disconnecting client due to handshake error: " + errorReason);
                this.listener.disconnect(Component.literal(errorReason));
                return;
            }

            System.out.println("[redox-mc-bridge] Client handshake validation successful! Advancing to PLAY phase.");
            this.finished = true;
        } catch (Exception e) {
            System.err.println("[redox-mc-bridge] Error parsing client handshake response: " + e.getMessage());
            this.listener.disconnect(Component.literal("Corrupted or invalid mod handshake response from client."));
        } finally {
            ModNetworkBridge.unregisterActiveHandshakeTask(this.listener);
        }
    }

    public boolean isFinished() {
        return this.finished;
    }

    @Override
    public ConfigurationTask.Type type() {
        return TYPE;
    }
}
