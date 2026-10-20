package net.redoxmc.bridge.resource;

import java.io.ByteArrayInputStream;
import java.io.InputStream;
import java.util.HashSet;
import java.util.Set;
import net.minecraft.resources.Identifier;
import net.minecraft.server.packs.AbstractPackMetadataResources;
import net.minecraft.server.packs.PackLocationInfo;
import net.minecraft.server.packs.PackResources;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.resources.IoSupplier;
import net.redoxmc.bridge.RedoxBridge;
import org.jspecify.annotations.Nullable;

/**
 * Virtual PackResources implementation serving memory streams directly from Redox-MC native loader.
 * Compatible with Minecraft 26.3 pack formats (97 for client resources, 121 for server data).
 */
public class VirtualModPackResources extends AbstractPackMetadataResources implements PackResources {

    public VirtualModPackResources(final PackLocationInfo location) {
        super(location);
    }

    @Override
    public @Nullable IoSupplier<InputStream> getRootResource(final String... path) {
        if (path == null || path.length == 0) {
            return null;
        }
        String fullPath = String.join("/", path);
        byte[] data = RedoxBridge.getVirtualRootResource(fullPath);
        if (data != null) {
            return () -> new ByteArrayInputStream(data);
        }
        return null;
    }

    @Override
    public @Nullable IoSupplier<InputStream> getResource(final PackType type, final Identifier location) {
        if (type == null || location == null) {
            return null;
        }
        byte[] data = RedoxBridge.getVirtualResource(type.getDirectory(), location.getNamespace(), location.getPath());
        if (data != null) {
            return () -> new ByteArrayInputStream(data);
        }
        return null;
    }

    @Override
    public void listResources(final PackType type, final String namespace, final String directory, final PackResources.ResourceOutput output) {
        if (type == null || namespace == null || directory == null || output == null) {
            return;
        }
        String[] relativePaths = RedoxBridge.listVirtualResources(type.getDirectory(), namespace, directory);
        if (relativePaths != null) {
            for (String relPath : relativePaths) {
                Identifier id = Identifier.tryBuild(namespace, relPath);
                if (id != null) {
                    byte[] data = RedoxBridge.getVirtualResource(type.getDirectory(), namespace, relPath);
                    if (data != null) {
                        output.accept(id, () -> new ByteArrayInputStream(data));
                    }
                }
            }
        }
    }

    @Override
    public Set<String> getNamespaces(final PackType type) {
        Set<String> namespaces = new HashSet<>();
        if (type != null) {
            String[] list = RedoxBridge.getVirtualNamespaces(type.getDirectory());
            if (list != null) {
                for (String ns : list) {
                    namespaces.add(ns);
                }
            }
        }
        return namespaces;
    }

    @Override
    public void close() {
        // Virtual in-memory pack, no external filesystem resources to release
    }
}
