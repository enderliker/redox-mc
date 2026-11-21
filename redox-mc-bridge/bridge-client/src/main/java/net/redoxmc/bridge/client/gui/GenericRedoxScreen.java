package net.redoxmc.bridge.client.gui;

import net.redoxmc.bridge.gui.GenericRedoxMenu;

/**
 * Isolated client holder and abstraction for GenericRedoxMenu screens.
 * Decoupled from server classpath.
 */
public class GenericRedoxScreen {
    private final GenericRedoxMenu menu;
    private final String title;

    public GenericRedoxScreen(GenericRedoxMenu menu, String title) {
        this.menu = menu;
        this.title = (title != null) ? title : "";
    }

    public GenericRedoxMenu getMenu() {
        return this.menu;
    }

    public String getTitle() {
        return this.title;
    }

    public static boolean isScreenSupported() {
        return true;
    }
}
