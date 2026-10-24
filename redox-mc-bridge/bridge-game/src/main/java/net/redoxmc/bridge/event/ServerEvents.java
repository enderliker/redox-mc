package net.redoxmc.bridge.event;

import net.minecraft.commands.CommandSourceStack;
import net.minecraft.commands.Commands;
import net.minecraft.network.chat.Component;
import net.minecraft.network.protocol.game.ServerboundChatPacket;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.network.ServerGamePacketListenerImpl;
import net.redoxmc.bridge.RedoxBridge;
import net.redoxmc.bridge.network.ModNetworkBridge;

/**
 * Server gameplay lifecycle and packet event bridge.
 *
 * Resides in bridge-game and is defined into the game's ClassLoader,
 * allowing direct access to vanilla Minecraft classes without reflection.
 */
public final class ServerEvents {
    private static volatile MinecraftServer serverInstance;

    private ServerEvents() {}

    /**
     * Returns the most recently observed MinecraftServer instance, or null
     * before the first server tick. Used by the network bridge to send
     * clientbound mod payloads without relying on a static game singleton.
     */
    public static MinecraftServer getServer() {
        return serverInstance;
    }

    /**
     * Hook called from MinecraftServer.tickChildren() at ~20 Hz (every 50 ms).
     *
     * @param server The MinecraftServer instance executing the tick loop.
     */
    public static void onServerTick(MinecraftServer server) {
        if (server != null) {
            serverInstance = server;
        }
        ModNetworkBridge.sweepHandshakeTasks();
        long tickCount = (server != null) ? (long) server.getTickCount() : 0L;
        RedoxBridge.onServerTick(tickCount);
    }

    /**
     * Hook called when a player sends a chat message from ServerGamePacketListenerImpl.handleChat().
     *
     * @param listener The server packet listener representing the connection.
     * @param packet The incoming ServerboundChatPacket.
     * @return true to allow standard chat message delivery, false to cancel/drop the message.
     */
    public static boolean onHandleChat(ServerGamePacketListenerImpl listener, ServerboundChatPacket packet) {
        String playerName = (listener != null && listener.player != null)
                ? listener.player.getPlainTextName()
                : "unknown";
        String message = (packet != null) ? packet.message() : "";
        return RedoxBridge.onServerChat(playerName, message);
    }

    /**
     * Hook called at the entry of Commands.performPrefixedCommand().
     *
     * Handles Redox loader commands (currently {@code /redox reload}) and returns
     * false to skip vanilla Brigadier dispatch for those commands. Every other
     * command returns true and is executed unchanged.
     *
     * @param commands The command dispatcher instance (unused).
     * @param source The command source (console, player, command block, or RCON).
     * @param command The raw command string, with or without a leading slash.
     * @return true to let vanilla handle the command, false if it was handled here.
     */
    public static boolean onPerformPrefixedCommand(Commands commands, CommandSourceStack source, String command) {
        String normalized = (command != null) ? command.trim() : "";
        if (normalized.startsWith("/")) {
            normalized = normalized.substring(1).trim();
        }

        if (!normalized.equals("redox reload")) {
            return true;
        }

        if (source != null && source.permissions() != null && !Commands.LEVEL_ADMINS.check(source.permissions())) {
            source.sendFailure(Component.literal("You do not have permission to execute this command."));
            return false;
        }

        String result;
        try {
            result = RedoxBridge.reloadMods();
        } catch (Throwable throwable) {
            result = "Redox reload failed: " + throwable.getMessage();
        }

        if (result == null || result.isEmpty()) {
            result = "Redox reload failed: native bridge returned null or empty result.";
        }

        final String message = result;
        if (source != null) {
            if (message.startsWith("Redox reload failed")) {
                source.sendFailure(Component.literal(message));
            } else {
                source.sendSuccess(() -> Component.literal(message), false);
            }
        } else {
            System.out.println("[redox-mc-bridge] " + message);
        }
        return false;
    }
}
