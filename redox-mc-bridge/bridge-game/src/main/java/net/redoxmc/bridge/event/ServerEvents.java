package net.redoxmc.bridge.event;

import net.minecraft.network.protocol.game.ServerboundChatPacket;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.network.ServerGamePacketListenerImpl;
import net.redoxmc.bridge.RedoxBridge;

/**
 * Server gameplay lifecycle and packet event bridge.
 *
 * Resides in bridge-game and is defined into the game's ClassLoader,
 * allowing direct access to vanilla Minecraft classes without reflection.
 */
public final class ServerEvents {
    private ServerEvents() {}

    /**
     * Hook called from MinecraftServer.tickChildren() at ~20 Hz (every 50 ms).
     *
     * @param server The MinecraftServer instance executing the tick loop.
     */
    public static void onServerTick(MinecraftServer server) {
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
}
