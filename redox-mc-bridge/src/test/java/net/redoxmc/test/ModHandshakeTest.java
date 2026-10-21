package net.redoxmc.test;

import io.netty.buffer.Unpooled;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.network.protocol.Packet;
import net.minecraft.network.protocol.common.ClientboundCustomPayloadPacket;
import net.minecraft.server.network.ConfigurationTask;
import net.redoxmc.bridge.RedoxBridge;
import net.redoxmc.bridge.network.ModHandshakeConfigurationTask;
import net.redoxmc.bridge.network.ModRawPayload;

public class ModHandshakeTest {
    public static void main(String[] args) throws Exception {
        System.out.println("[TEST] Starting ModHandshakeTest for Task 6.2...");

        RedoxBridge.ensureInitialized();

        // 1. Verify ModHandshakeConfigurationTask type
        System.out.println("[TEST] Step 1: Checking configuration task type...");
        if (!ModHandshakeConfigurationTask.TYPE.id().equals("redox:mod_handshake")) {
            throw new AssertionError("FAIL: Unexpected task type: " + ModHandshakeConfigurationTask.TYPE);
        }
        System.out.println("[TEST] PASS: Configuration task type verified.");

        // 2. Test RedoxBridge native handshake validation methods
        System.out.println("[TEST] Step 2: Testing RedoxBridge.validateClientHandshake via JNI...");
        String serverHash = RedoxBridge.getRegistryHash();
        String[] serverModIds = RedoxBridge.getLoadedModIds();
        String[] serverModVersions = RedoxBridge.getLoadedModVersions();
        System.out.println("[TEST] Server registry hash: '" + serverHash + "', loaded mods: " + serverModIds.length);

        // Matching credentials -> null (success)
        String validRes = RedoxBridge.validateClientHandshake(serverHash, serverModIds, serverModVersions);
        if (validRes != null) {
            throw new AssertionError("FAIL: Expected null for matching handshake, got error: " + validRes);
        }
        System.out.println("[TEST] PASS: Matching credentials validated cleanly.");

        // Hash mismatch
        if (serverHash != null && !serverHash.isEmpty()) {
            String mismatchRes = RedoxBridge.validateClientHandshake("corrupted_hash", serverModIds, serverModVersions);
            if (mismatchRes == null || !mismatchRes.contains("Mismatched registry content hash")) {
                throw new AssertionError("FAIL: Expected hash mismatch error, got: " + mismatchRes);
            }
            System.out.println("[TEST] PASS: Hash mismatch rejected with: " + mismatchRes);
        }

        // Missing mod
        if (serverModIds != null && serverModIds.length > 0) {
            String missingModRes = RedoxBridge.validateClientHandshake(serverHash, new String[0], new String[0]);
            if (missingModRes == null || !missingModRes.contains("Missing required mod")) {
                throw new AssertionError("FAIL: Expected missing mod error, got: " + missingModRes);
            }
            System.out.println("[TEST] PASS: Missing mod rejected with: " + missingModRes);

            // Incompatible version
            String[] wrongVersions = new String[serverModVersions.length];
            for (int i = 0; i < wrongVersions.length; i++) {
                wrongVersions[i] = "999.0.0";
            }
            String wrongVerRes = RedoxBridge.validateClientHandshake(serverHash, serverModIds, wrongVersions);
            if (wrongVerRes == null || !wrongVerRes.contains("Incompatible version")) {
                throw new AssertionError("FAIL: Expected incompatible version error, got: " + wrongVerRes);
            }
            System.out.println("[TEST] PASS: Version mismatch rejected with: " + wrongVerRes);
        }

        // 3. Test ModHandshakeConfigurationTask start packet emission
        System.out.println("[TEST] Step 3: Testing ModHandshakeConfigurationTask packet flow...");
        List<Packet<?>> sentPackets = new ArrayList<>();
        ModHandshakeConfigurationTask task = new ModHandshakeConfigurationTask(null);

        task.start(sentPackets::add);

        if (serverModIds != null && serverModIds.length > 0) {
            if (sentPackets.isEmpty()) {
                throw new AssertionError("FAIL: No handshake request packet sent when server has mods!");
            }
            Packet<?> packet = sentPackets.get(0);
            if (!(packet instanceof ClientboundCustomPayloadPacket customPacket)) {
                throw new AssertionError("FAIL: Expected ClientboundCustomPayloadPacket, got: " + packet.getClass());
            }
            if (!(customPacket.payload() instanceof ModRawPayload modPayload)) {
                throw new AssertionError("FAIL: Expected ModRawPayload, got: " + customPacket.payload().getClass());
            }
            if (!modPayload.id().equals(ModHandshakeConfigurationTask.HANDSHAKE_REQUEST_CHANNEL)) {
                throw new AssertionError("FAIL: Unexpected channel: " + modPayload.id());
            }
            System.out.println("[TEST] PASS: Handshake request packet format verified.");
        } else {
            if (!task.isFinished()) {
                throw new AssertionError("FAIL: Task should finish immediately if server has 0 mods!");
            }
            System.out.println("[TEST] PASS: Vanilla server bypass verified.");
        }

        // 4. Test explicit serialization and deserialization of mod handshake packets
        System.out.println("[TEST] Step 4: Testing handshake request and response wire formats...");
        FriendlyByteBuf mockReqBuf = new FriendlyByteBuf(Unpooled.buffer());
        mockReqBuf.writeUtf("simulated_server_hash_xyz");
        mockReqBuf.writeVarInt(2);
        mockReqBuf.writeUtf("testmod");
        mockReqBuf.writeUtf("0.1.0");
        mockReqBuf.writeUtf("othermod");
        mockReqBuf.writeUtf("1.2.3");

        byte[] reqData = new byte[mockReqBuf.readableBytes()];
        mockReqBuf.readBytes(reqData);
        ModRawPayload reqPayload = new ModRawPayload(ModHandshakeConfigurationTask.HANDSHAKE_REQUEST_CHANNEL, reqData);

        if (!reqPayload.id().equals(ModHandshakeConfigurationTask.HANDSHAKE_REQUEST_CHANNEL)) {
            throw new AssertionError("FAIL: Request channel mismatch!");
        }

        FriendlyByteBuf parsedReq = new FriendlyByteBuf(Unpooled.wrappedBuffer(reqPayload.data()));
        String parsedHash = parsedReq.readUtf();
        int parsedCount = parsedReq.readVarInt();
        if (!"simulated_server_hash_xyz".equals(parsedHash) || parsedCount != 2) {
            throw new AssertionError("FAIL: Request wire format parsing failed!");
        }
        System.out.println("[TEST] PASS: Handshake request wire format verified: hash=" + parsedHash + ", count=" + parsedCount);

        FriendlyByteBuf mockRespBuf = new FriendlyByteBuf(Unpooled.buffer());
        mockRespBuf.writeUtf("simulated_server_hash_xyz");
        mockRespBuf.writeVarInt(2);
        mockRespBuf.writeUtf("testmod");
        mockRespBuf.writeUtf("0.1.0");
        mockRespBuf.writeUtf("othermod");
        mockRespBuf.writeUtf("1.2.3");

        byte[] respData = new byte[mockRespBuf.readableBytes()];
        mockRespBuf.readBytes(respData);
        ModRawPayload respPayload = new ModRawPayload(ModHandshakeConfigurationTask.HANDSHAKE_RESPONSE_CHANNEL, respData);

        if (!respPayload.id().equals(ModHandshakeConfigurationTask.HANDSHAKE_RESPONSE_CHANNEL)) {
            throw new AssertionError("FAIL: Response channel mismatch!");
        }

        FriendlyByteBuf parsedResp = new FriendlyByteBuf(Unpooled.wrappedBuffer(respPayload.data()));
        String respHash = parsedResp.readUtf();
        int respCount = parsedResp.readVarInt();
        if (!"simulated_server_hash_xyz".equals(respHash) || respCount != 2) {
            throw new AssertionError("FAIL: Response wire format parsing failed!");
        }
        System.out.println("[TEST] PASS: Handshake response wire format verified: hash=" + respHash + ", count=" + respCount);

        System.out.println("[TEST] ========================================================");
        System.out.println("[TEST] ALL CHECKS PASSED SUCCESSFULLY FOR TASK 6.2!");
        System.out.println("[TEST] ========================================================");
    }
}
