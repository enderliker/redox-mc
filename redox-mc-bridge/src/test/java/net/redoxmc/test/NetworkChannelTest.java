package net.redoxmc.test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import net.redoxmc.bridge.RedoxBridge;
import net.redoxmc.bridge.network.ModNetworkBridge;

/**
 * End-to-end test for Task 6.3: typed Rust network channels with sender identity.
 *
 * The test runs with the native agent and a mods/test_mod directory in the
 * process working directory. It exercises the real native dispatch and outbox
 * drain paths; outbound packets are captured before they reach a live connection.
 */
public class NetworkChannelTest {
    private static final String PLAYER_NAME = "TestPlayer";
    private static final String PLAYER_UUID = "11111111-1111-1111-1111-111111111111";

    public static void main(String[] args) throws Exception {
        System.out.println("[TEST] Starting NetworkChannelTest for Task 6.3...");

        // Capture the game classloader with a lightweight vanilla class so the
        // bridge-game classes can be defined before the test references them.
        Class.forName("net.minecraft.network.FriendlyByteBuf");
        RedoxBridge.ensureInitialized();

        List<Object[]> outbound = new ArrayList<>();
        ModNetworkBridge.setTestCaptureSink(outbound::add);

        // Load the native test mod from ./mods/test_mod and register its channels.
        RedoxBridge.onBootstrapEntry();

        // 1. A server tick makes the example mod broadcast a ping.
        System.out.println("[TEST] Step 1: server tick emits a broadcast ping...");
        outbound.clear();
        RedoxBridge.onServerTick(1234L);

        Object[] ping = findMessage(outbound, "testmod:ping");
        if (ping == null) {
            throw new AssertionError("FAIL: No testmod:ping message was captured. Captured channels: " + channels(outbound));
        }
        int pingTarget = (Integer) ping[1];
        if (pingTarget != RedoxBridge.NETWORK_TARGET_BROADCAST) {
            throw new AssertionError("FAIL: Ping target should be BROADCAST, got " + pingTarget);
        }
        long nonce = readNonce((byte[]) ping[4]);
        if (nonce != 1234L) {
            throw new AssertionError("FAIL: Expected ping nonce 1234, got " + nonce);
        }
        System.out.println("[TEST] PASS: Captured broadcast ping with nonce " + nonce + ".");

        // 2. Simulate the client pong. The Rust handler validates the nonce and
        //    sends an ack back to the sender identity carried by the callback.
        System.out.println("[TEST] Step 2: simulated player pong is validated and acked...");
        outbound.clear();
        RedoxBridge.onModCustomPayloadFrom(
            "testmod:pong",
            pongBytes(nonce, PLAYER_NAME),
            PLAYER_NAME,
            PLAYER_UUID,
            RedoxBridge.NETWORK_SIDE_SERVER
        );

        Object[] ack = findMessage(outbound, "testmod:ack");
        if (ack == null) {
            throw new AssertionError("FAIL: No testmod:ack message was captured. Captured channels: " + channels(outbound));
        }
        int ackTarget = (Integer) ack[1];
        if (ackTarget != RedoxBridge.NETWORK_TARGET_PLAYER) {
            throw new AssertionError("FAIL: Ack target should be PLAYER, got " + ackTarget);
        }
        if (!PLAYER_NAME.equals(ack[2])) {
            throw new AssertionError("FAIL: Ack target name should be '" + PLAYER_NAME + "', got '" + ack[2] + "'");
        }
        if (!PLAYER_UUID.equals(ack[3])) {
            throw new AssertionError("FAIL: Ack target UUID should be '" + PLAYER_UUID + "', got '" + ack[3] + "'");
        }
        AckPayload ackPayload = parseAck((byte[]) ack[4]);
        if (ackPayload.nonce != nonce) {
            throw new AssertionError("FAIL: Ack nonce mismatch: " + ackPayload.nonce + " != " + nonce);
        }
        if (!PLAYER_NAME.equals(ackPayload.responder)) {
            throw new AssertionError("FAIL: Ack responder mismatch: " + ackPayload.responder);
        }
        System.out.println("[TEST] PASS: Pong validated, ack targeted at " + ack[2] + " (" + ack[3] + ").");

        // 3. Client-originated ping: the shared Rust handler replies with a pong
        //    addressed to the remote server.
        System.out.println("[TEST] Step 3: client-originated ping replies to the server...");
        outbound.clear();
        long clientNonce = 777L;
        RedoxBridge.onModCustomPayloadFrom(
            "testmod:ping",
            pingBytes(clientNonce),
            "server",
            "",
            RedoxBridge.NETWORK_SIDE_CLIENT
        );

        Object[] pong = findMessage(outbound, "testmod:pong");
        if (pong == null) {
            throw new AssertionError("FAIL: No testmod:pong reply was captured. Captured channels: " + channels(outbound));
        }
        int pongTarget = (Integer) pong[1];
        if (pongTarget != RedoxBridge.NETWORK_TARGET_SERVER) {
            throw new AssertionError("FAIL: Pong target should be SERVER, got " + pongTarget);
        }
        AckPayload pongPayload = parseAck((byte[]) pong[4]);
        if (pongPayload.nonce != clientNonce) {
            throw new AssertionError("FAIL: Pong nonce mismatch: " + pongPayload.nonce + " != " + clientNonce);
        }
        if (!"server".equals(pongPayload.responder)) {
            throw new AssertionError("FAIL: Pong responder should be 'server', got '" + pongPayload.responder + "'");
        }
        System.out.println("[TEST] PASS: Client-to-server pong reply verified.");

        // 4. Typed channel validation rejects reserved/vanilla identifiers at the
        //    Rust layer; this just documents the contract checked by cargo tests.
        ModNetworkBridge.clearTestCaptureSink();
        RedoxBridge.onServerTick(1235L);

        System.out.println("[TEST] ========================================================");
        System.out.println("[TEST] ALL CHECKS PASSED SUCCESSFULLY FOR TASK 6.3!");
        System.out.println("[TEST] ========================================================");
    }

    private static Object[] findMessage(List<Object[]> messages, String channel) {
        for (Object[] message : messages) {
            if (channel.equals(message[0])) {
                return message;
            }
        }
        return null;
    }

    private static String channels(List<Object[]> messages) {
        List<String> names = new ArrayList<>();
        for (Object[] message : messages) {
            names.add(String.valueOf(message[0]));
        }
        return names.toString();
    }

    private static byte[] pingBytes(long nonce) {
        byte[] data = new byte[8];
        for (int i = 0; i < 8; i++) {
            data[i] = (byte) ((nonce >>> (8 * i)) & 0xFF);
        }
        return data;
    }

    private static byte[] pongBytes(long nonce, String responder) {
        byte[] suffix = responder.getBytes(StandardCharsets.UTF_8);
        byte[] data = new byte[8 + suffix.length];
        for (int i = 0; i < 8; i++) {
            data[i] = (byte) ((nonce >>> (8 * i)) & 0xFF);
        }
        System.arraycopy(suffix, 0, data, 8, suffix.length);
        return data;
    }

    private static long readNonce(byte[] data) {
        if (data.length < 8) {
            throw new AssertionError("FAIL: Payload is shorter than 8 bytes: " + data.length);
        }
        long value = 0;
        for (int i = 0; i < 8; i++) {
            value |= ((long) data[i] & 0xFFL) << (8 * i);
        }
        return value;
    }

    private static AckPayload parseAck(byte[] data) {
        if (data.length < 8) {
            throw new AssertionError("FAIL: Ack payload is shorter than 8 bytes: " + data.length);
        }
        return new AckPayload(readNonce(data), new String(data, 8, data.length - 8, StandardCharsets.UTF_8));
    }

    private record AckPayload(long nonce, String responder) {}
}
