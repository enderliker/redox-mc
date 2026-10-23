package net.redoxmc.bridge.network;

import java.util.function.Consumer;
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
    private volatile boolean finished = false;
    private int ticksWaiting = 0;
    private static final int TIMEOUT_TICKS = 20 * 15; // 15 seconds

    public ModHandshakeConfigurationTask(final ServerConfigurationPacketListenerImpl listener) {
        this.listener = listener;
    }

    @Override
    public void start(final Consumer<Packet<?>> connection) {
        if (this.finished) {
            return;
        }

        final ModHandshakePayload local;
        try {
            local = ModHandshakePayload.local();
        } catch (Exception | LinkageError e) {
            failClosed("Could not build the server mod handshake manifest: " + e.getMessage());
            return;
        }

        // If the server has no custom registry content and no side=both mods,
        // vanilla clients can join without the Redox-MC agent.
        if (local.registryHash().isEmpty() && local.modCount() == 0) {
            System.out.println("[redox-mc-bridge] No custom mods or registry content on server; skipping handshake.");
            this.finished = true;
            return;
        }

        ModNetworkBridge.registerActiveHandshakeTask(this.listener, this);

        try {
            byte[] payloadBytes = local.encode();
            if (payloadBytes.length > ModRawPayload.MAX_PAYLOAD_SIZE) {
                failClosed("Server mod handshake request exceeds the 1 MiB clientbound limit.");
                return;
            }
            System.out.println(
                "[redox-mc-bridge] Sending mod handshake request to client (hash="
                    + local.registryHash() + ", mods=" + local.modCount() + ")..."
            );
            connection.accept(new ClientboundCustomPayloadPacket(new ModRawPayload(HANDSHAKE_REQUEST_CHANNEL, payloadBytes)));
        } catch (Exception | LinkageError e) {
            failClosed("Could not send the mod handshake request: " + e.getMessage());
        }
    }

    @Override
    public boolean tick() {
        if (this.finished) {
            return true;
        }

        this.ticksWaiting++;
        if (this.ticksWaiting > TIMEOUT_TICKS) {
            System.err.println("[redox-mc-bridge] Mod handshake timed out waiting for client response.");
            failClosed("Mod handshake timed out waiting for client response. Ensure Redox-MC is installed on the client.");
            return true;
        }

        return false;
    }

    public void handleClientResponse(final byte[] data) {
        if (this.finished) {
            return;
        }

        try {
            ModHandshakePayload response = ModHandshakePayload.decode(data);

            // Validate against the server-side snapshot in Rust via JNI.
            String errorReason = RedoxBridge.validateClientHandshake(
                response.registryHash(),
                response.modIds(),
                response.modVersions()
            );
            if (errorReason != null && !errorReason.isEmpty()) {
                System.err.println("[redox-mc-bridge] Disconnecting client due to handshake error: " + errorReason);
                failClosed(errorReason);
                return;
            }

            System.out.println("[redox-mc-bridge] Client handshake validation successful! Advancing to PLAY phase.");
            this.finished = true;
        } catch (Exception | LinkageError e) {
            System.err.println("[redox-mc-bridge] Error parsing client handshake response: " + e.getMessage());
            failClosed("Corrupted or invalid mod handshake response from client.");
        } finally {
            ModNetworkBridge.unregisterActiveHandshakeTask(this.listener);
        }
    }

    public boolean isFinished() {
        return this.finished;
    }

    private void failClosed(final String reason) {
        this.finished = true;
        ModNetworkBridge.unregisterActiveHandshakeTask(this.listener);
        if (this.listener != null) {
            try {
                this.listener.disconnect(Component.literal(reason));
            } catch (Exception | LinkageError e) {
                System.err.println("[redox-mc-bridge] Failed to disconnect client after handshake failure: "
                    + e.getMessage());
            }
        } else {
            System.err.println("[redox-mc-bridge] Handshake failed: " + reason);
        }
    }

    @Override
    public ConfigurationTask.Type type() {
        return TYPE;
    }
}
