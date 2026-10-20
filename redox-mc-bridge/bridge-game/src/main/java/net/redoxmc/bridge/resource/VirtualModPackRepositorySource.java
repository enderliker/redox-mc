package net.redoxmc.bridge.resource;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.function.Consumer;
import java.util.stream.Stream;
import net.minecraft.network.chat.Component;
import net.minecraft.server.packs.PackLocationInfo;
import net.minecraft.server.packs.PackMetadataResources;
import net.minecraft.server.packs.PackResources;
import net.minecraft.server.packs.PackSelectionConfig;
import net.minecraft.server.packs.repository.Pack;
import net.minecraft.server.packs.repository.PackCompatibility;
import net.minecraft.server.packs.repository.PackSource;
import net.minecraft.server.packs.repository.RepositorySource;
import net.minecraft.world.flag.FeatureFlagSet;

/**
 * Virtual RepositorySource that supplies Redox-MC virtual mod resource/data packs to Minecraft.
 */
public class VirtualModPackRepositorySource implements RepositorySource {
    public static final String PACK_ID = "redox_mod_resources";

    public static final PackLocationInfo LOCATION = new PackLocationInfo(
        PACK_ID,
        Component.literal("Redox Virtual Mod Resources"),
        PackSource.BUILT_IN,
        Optional.empty()
    );

    public static final PackSelectionConfig SELECTION_CONFIG = new PackSelectionConfig(
        true, // required: automatically selected in rebuildSelected
        Pack.Position.BOTTOM,
        false
    );

    @Override
    public void loadPacks(final Consumer<Pack> result) {
        Pack pack = createVirtualPack();
        if (pack != null) {
            result.accept(pack);
        }
    }

    /**
     * Constructs a Pack instance wrapping VirtualModPackResources.
     */
    public static Pack createVirtualPack() {
        Pack.ResourcesSupplier supplier = new Pack.ResourcesSupplier() {
            @Override
            public PackMetadataResources openMetadata(final PackLocationInfo location) {
                return new VirtualModPackResources(location);
            }

            @Override
            public Stream<PackResources> openResources(final PackLocationInfo location, final Pack.Metadata metadata) {
                return Stream.of(new VirtualModPackResources(location));
            }
        };

        Pack.Metadata metadata = new Pack.Metadata(
            Component.literal("Virtual assets and data packs provided by Redox native mods"),
            PackCompatibility.COMPATIBLE,
            FeatureFlagSet.of(),
            List.of()
        );

        return new Pack(LOCATION, supplier, metadata, SELECTION_CONFIG);
    }

    /**
     * Hook called right before PackRepository.discoverAvailable$original() executes.
     */
    public static void onPreDiscoverPacks() {
        // Trace / hook point before discovery
    }

    /**
     * Post-hook called after PackRepository.discoverAvailable$original() returns.
     * Injects the virtual pack into the discovered packs map.
     */
    public static Map<String, Pack> injectVirtualPack(final Map<String, Pack> discovered) {
        if (discovered != null && discovered.containsKey(PACK_ID)) {
            return discovered;
        }

        Map<String, Pack> result = new LinkedHashMap<>();
        if (discovered != null) {
            result.putAll(discovered);
        }

        Pack virtualPack = createVirtualPack();
        result.put(PACK_ID, virtualPack);
        return Map.copyOf(result);
    }
}
