package net.redoxmc.test;

import java.util.ArrayList;
import java.util.List;
import net.minecraft.network.protocol.Packet;
import net.minecraft.network.protocol.common.ServerboundCustomPayloadPacket;
import net.redoxmc.bridge.RedoxBridge;
import net.redoxmc.bridge.network.ModHandshakeConfigurationTask;
import net.redoxmc.bridge.network.ModHandshakePayload;
import net.redoxmc.bridge.network.ModNetworkBridge;
import net.redoxmc.bridge.network.ModRawPayload;

/**
 * Integration test for the client side of the CONFIGURATION-phase handshake.
 *
 * <p>The bridge-game classes are compiled against the server jar, so this test
 * supplies a public listener stub with a {@code send(Packet)} method. That is
 * exactly the reflective contract used by {@code respondToHandshakeRequest} on
 * a real {@code ClientConfigurationPacketListenerImpl}.</p>
 */
public class ClientHandshakeTest {
    public static void main(String[] args) throws Exception {
        System.out.println("[TEST] Starting ClientHandshakeTest...");

        RedoxBridge.ensureInitialized();
        RedoxBridge.onBootstrapEntry();

        ModHandshakePayload local = ModHandshakePayload.local();

        // 1. A handshake request is answered directly through the listener.
        System.out.println("[TEST] Step 1: Answering a handshake request through the listener...");
        StubListener listener = new StubListener();
        ModNetworkBridge.onClientCustomPayload(
            listener,
            new ModRawPayload(
                ModHandshakeConfigurationTask.HANDSHAKE_REQUEST_CHANNEL,
                new byte[] { 0x01, 0x02, 0x03 }
            )
        );

        if (listener.sent.size() != 1) {
            throw new AssertionError("FAIL: Expected exactly one response, got " + listener.sent.size());
        }
        Packet<?> responsePacket = listener.sent.get(0);
        if (!(responsePacket instanceof ServerboundCustomPayloadPacket serverbound)) {
            throw new AssertionError("FAIL: Expected ServerboundCustomPayloadPacket, got "
                + responsePacket.getClass());
        }
        if (!(serverbound.payload() instanceof ModRawPayload responsePayload)) {
            throw new AssertionError("FAIL: Expected ModRawPayload, got "
                + serverbound.payload().getClass());
        }
        if (!responsePayload.id().equals(ModHandshakeConfigurationTask.HANDSHAKE_RESPONSE_CHANNEL)) {
            throw new AssertionError("FAIL: Unexpected response channel: " + responsePayload.id());
        }

        ModHandshakePayload response = ModHandshakePayload.decode(responsePayload.data());
        if (!response.registryHash().equals(local.registryHash())) {
            throw new AssertionError(
                "FAIL: Response hash '" + response.registryHash()
                    + "' does not match local hash '" + local.registryHash() + "'"
            );
        }
        if (response.modCount() != local.modCount()) {
            throw new AssertionError(
                "FAIL: Response mod count " + response.modCount()
                    + " does not match local count " + local.modCount()
            );
        }
        String[] responseIds = response.modIds();
        String[] localIds = local.modIds();
        for (int i = 0; i < local.modCount(); i++) {
            if (!responseIds[i].equals(localIds[i])) {
                throw new AssertionError("FAIL: Response mod id " + i + " does not match");
            }
        }
        if (responsePayload.data().length > 32767) {
            throw new AssertionError("FAIL: Response exceeds the serverbound limit");
        }
        System.out.println("[TEST] PASS: Client handshake response verified (hash='"
            + response.registryHash() + "', mods=" + response.modCount() + ").");

        // 2. A missing listener must not throw or send anything.
        System.out.println("[TEST] Step 2: Verifying null-listener safety...");
        ModNetworkBridge.onClientCustomPayload(
            null,
            new ModRawPayload(
                ModHandshakeConfigurationTask.HANDSHAKE_REQUEST_CHANNEL,
                new byte[] { 0x01 }
            )
        );
        System.out.println("[TEST] PASS: Null listener is ignored safely.");

        // 3. Non-handshake mod payloads must not be answered with a handshake
        //    response. They are dispatched to native mod handlers instead.
        System.out.println("[TEST] Step 3: Verifying ordinary payloads are not answered...");
        StubListener ordinaryListener = new StubListener();
        ModNetworkBridge.onClientCustomPayload(
            ordinaryListener,
            new ModRawPayload(
                net.minecraft.resources.Identifier.fromNamespaceAndPath("testmod", "unhandled"),
                new byte[] { 0x09 }
            )
        );
        if (!ordinaryListener.sent.isEmpty()) {
            throw new AssertionError("FAIL: Ordinary payload must not produce a handshake response");
        }
        System.out.println("[TEST] PASS: Ordinary payloads are not answered.");

        System.out.println("[TEST] ========================================================");
        System.out.println("[TEST] ALL CLIENT HANDSHAKE CHECKS PASSED!");
        System.out.println("[TEST] ========================================================");
    }

    /** Public listener stub matching the reflective {@code send(Packet)} contract. */
    public static final class StubListener {
        private final List<Packet<?>> sent = new ArrayList<>();

        public void send(final Packet<?> packet) {
            this.sent.add(packet);
        }
    }
}
