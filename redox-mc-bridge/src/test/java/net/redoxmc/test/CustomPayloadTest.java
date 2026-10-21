package net.redoxmc.test;

import io.netty.buffer.Unpooled;
import java.util.Arrays;
import java.util.List;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.common.custom.BrandPayload;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;
import net.minecraft.network.protocol.common.custom.DiscardedPayload;
import net.minecraft.resources.Identifier;
import net.redoxmc.bridge.RedoxBridge;
import net.redoxmc.bridge.network.ModNetworkBridge;
import net.redoxmc.bridge.network.ModRawPayload;

public class CustomPayloadTest {
    public static void main(String[] args) throws Exception {
        System.out.println("[TEST] Starting CustomPayloadTest for Task 6.1...");

        RedoxBridge.ensureInitialized();

        // 1. Direct ModRawPayload serialization & deserialization with FriendlyByteBuf
        System.out.println("[TEST] Step 1: Testing ModRawPayload encoding/decoding via FriendlyByteBuf...");
        Identifier modChannelId = Identifier.fromNamespaceAndPath("testmod", "telemetry");
        byte[] expectedBytes = new byte[] { 0x10, 0x20, 0x30, 0x40, (byte) 0xFF, (byte) 0xEE };
        ModRawPayload originalPayload = new ModRawPayload(modChannelId, expectedBytes);

        StreamCodec<FriendlyByteBuf, ModRawPayload> payloadCodec = ModRawPayload.codec(modChannelId);
        FriendlyByteBuf buffer = new FriendlyByteBuf(Unpooled.buffer());

        payloadCodec.encode(buffer, originalPayload);
        if (buffer.readableBytes() != expectedBytes.length) {
            throw new AssertionError(
                "FAIL: Encoded buffer length (" + buffer.readableBytes() + ") does not match payload length (" + expectedBytes.length + ")"
            );
        }

        ModRawPayload decodedPayload = payloadCodec.decode(buffer);
        if (!decodedPayload.id().equals(modChannelId)) {
            throw new AssertionError("FAIL: Decoded channel ID does not match: " + decodedPayload.id());
        }
        if (!Arrays.equals(decodedPayload.data(), expectedBytes)) {
            throw new AssertionError("FAIL: Decoded payload bytes do not match original bytes!");
        }
        if (buffer.readableBytes() != 0) {
            throw new AssertionError("FAIL: Buffer has unread remaining bytes after decode!");
        }
        System.out.println("[TEST] PASS: ModRawPayload direct codec verified.");

        // 2. Wrap CustomPacketPayload.codec and test vanilla BrandPayload passthrough
        System.out.println("[TEST] Step 2: Testing vanilla BrandPayload decoding through wrapCodec...");
        StreamCodec<FriendlyByteBuf, CustomPacketPayload> vanillaCodec = CustomPacketPayload.codec(
            id -> DiscardedPayload.codec(id, 32767),
            List.of(new CustomPacketPayload.TypeAndCodec<>(BrandPayload.TYPE, BrandPayload.STREAM_CODEC))
        );
        StreamCodec<FriendlyByteBuf, CustomPacketPayload> wrappedCodec = ModNetworkBridge.wrapCodec(vanillaCodec);

        FriendlyByteBuf vanillaBuffer = new FriendlyByteBuf(Unpooled.buffer());
        BrandPayload originalBrand = new BrandPayload("vanilla-server");
        wrappedCodec.encode(vanillaBuffer, originalBrand);

        CustomPacketPayload decodedBrand = wrappedCodec.decode(vanillaBuffer);
        if (!(decodedBrand instanceof BrandPayload brandResult)) {
            throw new AssertionError(
                "FAIL: Expected BrandPayload, but got: " + (decodedBrand == null ? "null" : decodedBrand.getClass().getName())
            );
        }
        if (!"vanilla-server".equals(brandResult.brand())) {
            throw new AssertionError("FAIL: Brand name mismatch: " + brandResult.brand());
        }
        System.out.println("[TEST] PASS: Vanilla BrandPayload passes through wrapCodec unaltered.");

        // 3. Test Mod channel payload retention through wrapCodec
        System.out.println("[TEST] Step 3: Testing mod channel payload through wrapCodec...");
        FriendlyByteBuf modBuffer = new FriendlyByteBuf(Unpooled.buffer());
        Identifier customChannel = Identifier.fromNamespaceAndPath("testmod", "packet_data");
        byte[] modData = new byte[] { (byte) 0xCA, (byte) 0xFE, (byte) 0xBA, (byte) 0xBE, 1, 2, 3, 4, 5 };
        ModRawPayload sentPayload = new ModRawPayload(customChannel, modData);

        wrappedCodec.encode(modBuffer, sentPayload);

        CustomPacketPayload receivedPayload = wrappedCodec.decode(modBuffer);
        if (!(receivedPayload instanceof ModRawPayload modRaw)) {
            throw new AssertionError(
                "FAIL: Expected ModRawPayload for mod channel, but got: " +
                (receivedPayload == null ? "null" : receivedPayload.getClass().getName())
            );
        }
        if (!modRaw.id().equals(customChannel)) {
            throw new AssertionError("FAIL: Channel ID mismatch: " + modRaw.id());
        }
        if (!Arrays.equals(modRaw.data(), modData)) {
            throw new AssertionError("FAIL: Mod binary payload was corrupted or truncated!");
        }
        System.out.println("[TEST] PASS: Mod channel retained full binary buffer (" + modData.length + " bytes).");

        // 4. Test vanilla unknown payload falling back to DiscardedPayload
        System.out.println("[TEST] Step 4: Testing unknown vanilla channel falling back to DiscardedPayload...");
        FriendlyByteBuf unknownVanillaBuffer = new FriendlyByteBuf(Unpooled.buffer());
        Identifier unknownVanillaId = Identifier.fromNamespaceAndPath("minecraft", "unknown_packet");
        unknownVanillaBuffer.writeIdentifier(unknownVanillaId);
        unknownVanillaBuffer.writeBytes(new byte[] { 9, 8, 7, 6 });

        CustomPacketPayload unknownPayload = wrappedCodec.decode(unknownVanillaBuffer);
        if (!(unknownPayload instanceof DiscardedPayload discarded)) {
            throw new AssertionError(
                "FAIL: Expected DiscardedPayload for unknown vanilla channel, but got: " +
                (unknownPayload == null ? "null" : unknownPayload.getClass().getName())
            );
        }
        if (!discarded.id().equals(unknownVanillaId)) {
            throw new AssertionError("FAIL: DiscardedPayload ID mismatch: " + discarded.id());
        }
        System.out.println("[TEST] PASS: Unknown vanilla channel gracefully fell back to DiscardedPayload.");

        System.out.println("[TEST] ========================================================");
        System.out.println("[TEST] ALL CHECKS PASSED SUCCESSFULLY FOR TASK 6.1!");
        System.out.println("[TEST] ========================================================");
    }
}
