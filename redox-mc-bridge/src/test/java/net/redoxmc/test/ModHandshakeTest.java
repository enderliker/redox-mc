package net.redoxmc.test;

import io.netty.buffer.Unpooled;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.network.protocol.Packet;
import net.minecraft.network.protocol.common.ClientboundCustomPayloadPacket;
import net.redoxmc.bridge.RedoxBridge;
import net.redoxmc.bridge.network.ModHandshakeConfigurationTask;
import net.redoxmc.bridge.network.ModHandshakePayload;
import net.redoxmc.bridge.network.ModRawPayload;

/**
 * Integration test for the CONFIGURATION-phase mod handshake.
 *
 * <p>Run twice by the integration runner: once from a directory without
 * {@code mods/} (vanilla bypass path) and once from a directory containing the
 * example test mod (request/response path).</p>
 */
public class ModHandshakeTest {
    public static void main(String[] args) throws Exception {
        System.out.println("[TEST] Starting ModHandshakeTest...");

        RedoxBridge.ensureInitialized();
        RedoxBridge.onBootstrapEntry();

        // 1. Configuration task type.
        System.out.println("[TEST] Step 1: Checking configuration task type...");
        if (!ModHandshakeConfigurationTask.TYPE.id().equals("redox:mod_handshake")) {
            throw new AssertionError("FAIL: Unexpected task type: " + ModHandshakeConfigurationTask.TYPE);
        }
        System.out.println("[TEST] PASS: Configuration task type verified.");

        // 2. The native manifest is a consistent snapshot: [hash, id, version, ...].
        System.out.println("[TEST] Step 2: Verifying the native handshake manifest...");
        String[] manifest = RedoxBridge.getHandshakeManifest();
        if (manifest == null || manifest.length == 0) {
            throw new AssertionError("FAIL: getHandshakeManifest() returned an empty array");
        }
        if ((manifest.length - 1) % 2 != 0) {
            throw new AssertionError("FAIL: Manifest length is not 1 + 2n: " + manifest.length);
        }
        ModHandshakePayload local = ModHandshakePayload.local();
        if (!local.registryHash().equals(manifest[0])) {
            throw new AssertionError("FAIL: Manifest hash does not match local payload hash");
        }
        if (local.modCount() != (manifest.length - 1) / 2) {
            throw new AssertionError(
                "FAIL: Manifest mod count " + ((manifest.length - 1) / 2)
                    + " does not match payload count " + local.modCount()
            );
        }
        String[] localIds = local.modIds();
        String[] localVersions = local.modVersions();
        for (int i = 0; i < local.modCount(); i++) {
            if (!localIds[i].equals(manifest[1 + i * 2]) || !localVersions[i].equals(manifest[2 + i * 2])) {
                throw new AssertionError("FAIL: Manifest entry " + i + " does not match the payload");
            }
        }
        System.out.println(
            "[TEST] PASS: Manifest verified (hash='" + local.registryHash()
                + "', mods=" + local.modCount() + ")."
        );

        // 3. Round-trip through the shared wire format.
        System.out.println("[TEST] Step 3: Verifying handshake payload round-trip...");
        ModHandshakePayload decoded = ModHandshakePayload.decode(local.encode());
        if (!decoded.registryHash().equals(local.registryHash())
                || decoded.modCount() != local.modCount()) {
            throw new AssertionError("FAIL: Handshake payload round-trip mismatch");
        }
        System.out.println("[TEST] PASS: Handshake payload round-trip verified.");

        // 4. Native validation accepts the local credentials.
        System.out.println("[TEST] Step 4: Validating the local credentials through JNI...");
        String validationError = RedoxBridge.validateClientHandshake(
            local.registryHash(), local.modIds(), local.modVersions()
        );
        if (validationError != null) {
            throw new AssertionError("FAIL: Local credentials were rejected: " + validationError);
        }
        String lengthError = RedoxBridge.validateClientHandshake(
            local.registryHash(),
            local.modCount() == 0 ? new String[] { "extra" } : local.modIds(),
            new String[0]
        );
        if (lengthError == null || !lengthError.contains("differ in length")) {
            throw new AssertionError("FAIL: Expected a length mismatch error, got: " + lengthError);
        }
        System.out.println("[TEST] PASS: Native validation verified.");

        // 5. Negative validation cases that can be exercised with this server.
        System.out.println("[TEST] Step 5: Checking native rejection paths...");
        if (!local.registryHash().isEmpty()) {
            String mismatch = RedoxBridge.validateClientHandshake(
                "corrupted_hash", local.modIds(), local.modVersions()
            );
            if (mismatch == null || !mismatch.contains("Mismatched registry content hash")) {
                throw new AssertionError("FAIL: Expected hash mismatch error, got: " + mismatch);
            }
            System.out.println("[TEST] PASS: Hash mismatch rejected.");
        } else {
            System.out.println("[TEST] SKIP: No registry content, hash mismatch is not applicable.");
        }

        if (local.modCount() > 0) {
            String missing = RedoxBridge.validateClientHandshake(
                local.registryHash(), new String[0], new String[0]
            );
            if (missing == null || !missing.contains("Missing required mod")) {
                throw new AssertionError("FAIL: Expected missing mod error, got: " + missing);
            }

            String[] wrongVersions = new String[local.modCount()];
            for (int i = 0; i < wrongVersions.length; i++) {
                wrongVersions[i] = "999.0.0";
            }
            String wrongVersion = RedoxBridge.validateClientHandshake(
                local.registryHash(), local.modIds(), wrongVersions
            );
            if (wrongVersion == null || !wrongVersion.contains("Incompatible version")) {
                throw new AssertionError("FAIL: Expected version mismatch error, got: " + wrongVersion);
            }

            String[] duplicateIds = new String[local.modCount() + 1];
            String[] duplicateVersions = new String[local.modCount() + 1];
            for (int i = 0; i < local.modCount(); i++) {
                duplicateIds[i] = localIds[i];
                duplicateVersions[i] = localVersions[i];
            }
            duplicateIds[local.modCount()] = localIds[0];
            duplicateVersions[local.modCount()] = localVersions[0];
            String duplicate = RedoxBridge.validateClientHandshake(
                local.registryHash(), duplicateIds, duplicateVersions
            );
            if (duplicate == null || !duplicate.contains("duplicate mod")) {
                throw new AssertionError("FAIL: Expected duplicate mod error, got: " + duplicate);
            }
            System.out.println("[TEST] PASS: Missing, version-mismatch, and duplicate mods rejected.");
        } else {
            System.out.println("[TEST] SKIP: No mods loaded, mod rejection paths are not applicable.");
        }

        // 6. Task lifecycle: bypass when there is nothing to negotiate, otherwise
        //    emit exactly one request and accept the matching response.
        System.out.println("[TEST] Step 6: Exercising ModHandshakeConfigurationTask...");
        List<Packet<?>> sentPackets = new ArrayList<>();
        ModHandshakeConfigurationTask task = new ModHandshakeConfigurationTask(null);
        task.start(sentPackets::add);

        boolean needsHandshake = !local.registryHash().isEmpty() || local.modCount() > 0;
        if (!needsHandshake) {
            if (!task.isFinished()) {
                throw new AssertionError("FAIL: Task must finish immediately when there is nothing to negotiate");
            }
            if (!sentPackets.isEmpty()) {
                throw new AssertionError("FAIL: Vanilla bypass must not send a handshake request");
            }
            System.out.println("[TEST] PASS: Vanilla server bypass verified.");
        } else {
            if (task.isFinished()) {
                throw new AssertionError("FAIL: Task must wait for the client response");
            }
            if (sentPackets.size() != 1) {
                throw new AssertionError("FAIL: Expected exactly one request, got " + sentPackets.size());
            }
            if (!(sentPackets.get(0) instanceof ClientboundCustomPayloadPacket customPacket)
                    || !(customPacket.payload() instanceof ModRawPayload requestPayload)) {
                throw new AssertionError("FAIL: Request is not a ModRawPayload custom payload");
            }
            if (!requestPayload.id().equals(ModHandshakeConfigurationTask.HANDSHAKE_REQUEST_CHANNEL)) {
                throw new AssertionError("FAIL: Unexpected request channel: " + requestPayload.id());
            }
            ModHandshakePayload request = ModHandshakePayload.decode(requestPayload.data());
            if (!request.registryHash().equals(local.registryHash())
                    || request.modCount() != local.modCount()) {
                throw new AssertionError("FAIL: Request does not carry the local manifest");
            }

            task.handleClientResponse(local.encode());
            if (!task.isFinished()) {
                throw new AssertionError("FAIL: Task did not finish after a valid response");
            }
            System.out.println("[TEST] PASS: Request/response lifecycle verified.");
        }

        // 7. Timeout is fail-closed: tick() returns true and the task is finished.
        System.out.println("[TEST] Step 7: Verifying the handshake timeout...");
        ModHandshakeConfigurationTask timeoutTask = new ModHandshakeConfigurationTask(null);
        List<Packet<?>> timeoutPackets = new ArrayList<>();
        timeoutTask.start(timeoutPackets::add);
        if (needsHandshake) {
            boolean tickResult = false;
            for (int i = 0; i <= 301; i++) {
                tickResult = timeoutTask.tick();
            }
            if (!tickResult || !timeoutTask.isFinished()) {
                throw new AssertionError("FAIL: Timed-out task must report finished from tick()");
            }
            System.out.println("[TEST] PASS: Timeout is fail-closed.");
        } else {
            System.out.println("[TEST] SKIP: Bypass task finishes before the timeout applies.");
        }

        // 8. Malformed wire data must be rejected by the bounded decoder.
        System.out.println("[TEST] Step 8: Verifying malformed handshake rejection...");
        expectDecodeFailure(duplicateWire(), "duplicate mod id");
        expectDecodeFailure(trailingWire(local), "trailing bytes");
        expectDecodeFailure(tooManyModsWire(), "invalid handshake mod count");
        expectDecodeFailure(emptyIdWire(), "mod id must not be empty");
        expectDecodeFailure(oversizedHashWire(), "registry hash");
        System.out.println("[TEST] PASS: Malformed handshake payloads rejected.");

        System.out.println("[TEST] ========================================================");
        System.out.println("[TEST] ALL HANDSHAKE CHECKS PASSED!");
        System.out.println("[TEST] ========================================================");
    }

    private static byte[] duplicateWire() {
        FriendlyByteBuf buf = new FriendlyByteBuf(Unpooled.buffer());
        buf.writeUtf("hash");
        buf.writeVarInt(2);
        buf.writeUtf("testmod");
        buf.writeUtf("0.1.0");
        buf.writeUtf("testmod");
        buf.writeUtf("0.1.0");
        return drain(buf);
    }

    private static byte[] trailingWire(ModHandshakePayload local) {
        byte[] encoded = local.encode();
        byte[] withTrailing = new byte[encoded.length + 1];
        System.arraycopy(encoded, 0, withTrailing, 0, encoded.length);
        withTrailing[encoded.length] = 0x7F;
        return withTrailing;
    }

    private static byte[] tooManyModsWire() {
        FriendlyByteBuf buf = new FriendlyByteBuf(Unpooled.buffer());
        buf.writeUtf("hash");
        buf.writeVarInt(ModHandshakePayload.MAX_MODS + 1);
        return drain(buf);
    }

    private static byte[] emptyIdWire() {
        FriendlyByteBuf buf = new FriendlyByteBuf(Unpooled.buffer());
        buf.writeUtf("hash");
        buf.writeVarInt(1);
        buf.writeUtf("");
        buf.writeUtf("0.1.0");
        return drain(buf);
    }

    private static byte[] oversizedHashWire() {
        FriendlyByteBuf buf = new FriendlyByteBuf(Unpooled.buffer());
        buf.writeUtf("x".repeat(ModHandshakePayload.MAX_HASH_LENGTH + 1));
        buf.writeVarInt(0);
        return drain(buf);
    }

    private static byte[] drain(FriendlyByteBuf buf) {
        byte[] bytes = new byte[buf.readableBytes()];
        buf.readBytes(bytes);
        return bytes;
    }

    private static void expectDecodeFailure(byte[] data, String expectedMessage) {
        try {
            ModHandshakePayload.decode(data);
            throw new AssertionError("FAIL: Expected decode failure containing '" + expectedMessage + "'");
        } catch (IllegalArgumentException expected) {
            if (expected.getMessage() == null || !expected.getMessage().contains(expectedMessage)) {
                throw new AssertionError(
                    "FAIL: Expected message containing '" + expectedMessage
                        + "', got: " + expected.getMessage(),
                    expected
                );
            }
        }
    }
}
