package net.redoxmc.bridge.network;

import io.netty.buffer.Unpooled;
import java.util.HashSet;
import java.util.Objects;
import java.util.Set;
import net.minecraft.network.FriendlyByteBuf;
import net.redoxmc.bridge.RedoxBridge;

/**
 * Shared wire format for the CONFIGURATION-phase mod handshake.
 *
 * <p>Both the server request and the client response use the same layout:
 * {@code registryHash, modCount, (modId, modVersion)*}. Decoding is bounded to
 * avoid attacker-controlled allocations during a pre-authentication phase.</p>
 */
public final class ModHandshakePayload {
    /** Hard upper bound for the number of mods accepted in either direction. */
    public static final int MAX_MODS = 4096;
    /** Maximum length of the registry hash string. */
    public static final int MAX_HASH_LENGTH = 256;
    /** Maximum length of a mod identifier. */
    public static final int MAX_ID_LENGTH = 256;
    /** Maximum length of a mod version string. */
    public static final int MAX_VERSION_LENGTH = 128;

    private final String registryHash;
    private final String[] modIds;
    private final String[] modVersions;

    public ModHandshakePayload(final String registryHash, final String[] modIds, final String[] modVersions) {
        this.registryHash = Objects.requireNonNullElse(registryHash, "");
        Objects.requireNonNull(modIds, "modIds");
        Objects.requireNonNull(modVersions, "modVersions");
        if (modIds.length != modVersions.length) {
            throw new IllegalArgumentException("modIds and modVersions must have the same length");
        }
        if (modIds.length > MAX_MODS) {
            throw new IllegalArgumentException("too many mods in handshake payload: " + modIds.length);
        }
        if (this.registryHash.length() > MAX_HASH_LENGTH) {
            throw new IllegalArgumentException("registry hash is too long: " + this.registryHash.length());
        }
        for (int i = 0; i < modIds.length; i++) {
            validateString(modIds[i], MAX_ID_LENGTH, "mod id");
            validateString(modVersions[i], MAX_VERSION_LENGTH, "mod version");
        }
        this.modIds = modIds.clone();
        this.modVersions = modVersions.clone();
    }

    /**
     * Builds the local handshake payload from a single native snapshot.
     *
     * <p>Using one native call keeps the hash and the mod list consistent even
     * if mods are reloaded while the handshake is being built.</p>
     */
    public static ModHandshakePayload local() {
        String[] manifest = RedoxBridge.getHandshakeManifest();
        if (manifest == null || manifest.length == 0) {
            return new ModHandshakePayload("", new String[0], new String[0]);
        }
        if ((manifest.length - 1) % 2 != 0) {
            throw new IllegalStateException("Malformed handshake manifest returned by the loader");
        }
        int count = (manifest.length - 1) / 2;
        String[] ids = new String[count];
        String[] versions = new String[count];
        for (int i = 0; i < count; i++) {
            ids[i] = manifest[1 + i * 2];
            versions[i] = manifest[2 + i * 2];
        }
        return new ModHandshakePayload(manifest[0], ids, versions);
    }

    /**
     * Decodes and validates a handshake payload.
     *
     * @throws IllegalArgumentException when the payload is malformed or exceeds
     *                                  the bounded handshake limits
     */
    public static ModHandshakePayload decode(final byte[] data) {
        if (data == null) {
            throw new IllegalArgumentException("handshake payload must not be null");
        }
        FriendlyByteBuf buf = new FriendlyByteBuf(Unpooled.wrappedBuffer(data));
        String hash = readBoundedUtf(buf, MAX_HASH_LENGTH, "registry hash");
        int count = readModCount(buf);
        if (count < 0 || count > MAX_MODS) {
            throw new IllegalArgumentException("invalid handshake mod count: " + count);
        }
        // Each entry needs at least two length-prefixed strings (two bytes).
        if (count > buf.readableBytes() / 2) {
            throw new IllegalArgumentException(
                "handshake declares " + count + " mods but only " + buf.readableBytes() + " bytes remain"
            );
        }

        String[] ids = new String[count];
        String[] versions = new String[count];
        Set<String> seen = new HashSet<>(Math.max(16, count * 2));
        for (int i = 0; i < count; i++) {
            String id = readBoundedUtf(buf, MAX_ID_LENGTH, "mod id");
            String version = readBoundedUtf(buf, MAX_VERSION_LENGTH, "mod version");
            if (!seen.add(id)) {
                throw new IllegalArgumentException("duplicate mod id in handshake payload: " + id);
            }
            ids[i] = id;
            versions[i] = version;
        }
        if (buf.isReadable()) {
            throw new IllegalArgumentException("trailing bytes in handshake payload: " + buf.readableBytes());
        }
        return new ModHandshakePayload(hash, ids, versions);
    }

    /** Serializes this payload into the shared wire format. */
    public byte[] encode() {
        FriendlyByteBuf buf = new FriendlyByteBuf(Unpooled.buffer());
        buf.writeUtf(this.registryHash);
        buf.writeVarInt(this.modIds.length);
        for (int i = 0; i < this.modIds.length; i++) {
            buf.writeUtf(this.modIds[i]);
            buf.writeUtf(this.modVersions[i]);
        }
        byte[] bytes = new byte[buf.readableBytes()];
        buf.readBytes(bytes);
        return bytes;
    }

    public String registryHash() {
        return this.registryHash;
    }

    public int modCount() {
        return this.modIds.length;
    }

    public String[] modIds() {
        return this.modIds.clone();
    }

    public String[] modVersions() {
        return this.modVersions.clone();
    }

    private static String readBoundedUtf(final FriendlyByteBuf buf, final int maxLength, final String label) {
        try {
            return buf.readUtf(maxLength);
        } catch (RuntimeException e) {
            throw new IllegalArgumentException(label + " is invalid: " + e.getMessage(), e);
        }
    }

    private static int readModCount(final FriendlyByteBuf buf) {
        try {
            return buf.readVarInt();
        } catch (RuntimeException e) {
            throw new IllegalArgumentException("invalid handshake mod count: " + e.getMessage(), e);
        }
    }

    private static void validateString(final String value, final int maxLength, final String label) {
        if (value == null || value.isEmpty()) {
            throw new IllegalArgumentException(label + " must not be empty");
        }
        if (value.length() > maxLength) {
            throw new IllegalArgumentException(label + " is too long: " + value.length());
        }
    }
}
