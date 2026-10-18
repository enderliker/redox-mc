package net.redoxmc.test;

import net.redoxmc.bridge.RedoxBridge;
import java.io.File;
import java.net.URL;
import java.net.URLClassLoader;
import java.lang.reflect.Method;

public class BundlerSimulationTest {
    public static void main(String[] args) throws Exception {
        System.out.println("[TEST] Starting BundlerSimulationTest (simulating Mojang server bundler URLClassLoader)...");

        // The game classes directory where TestBundledMain.class resides and the real server jar
        File gameClassesDir = new File("redox-mc-bridge/build/test-game-classes");
        File serverJar = new File("versions/26.3/server-26.3.jar");
        URL[] urls = new URL[] { gameClassesDir.toURI().toURL(), serverJar.toURI().toURL() };

        // Mimic Mojang net.minecraft.bundler.Main: parent is PlatformClassLoader
        ClassLoader parent = ClassLoader.getPlatformClassLoader();
        URLClassLoader gameClassLoader = new URLClassLoader(urls, parent);

        System.out.println("[TEST] Created child URLClassLoader: " + gameClassLoader);
        System.out.println("[TEST] Parent is PlatformClassLoader: " + parent);

        // Before loading any game class, is classloader captured?
        // Note: AppClassLoader might have run main(), but no net/minecraft/* class has been loaded yet!
        if (RedoxBridge.isClassLoaderCaptured()) {
            System.out.println("[TEST] Warning: Classloader was already captured prior to test entry: " + RedoxBridge.getCapturedClassLoader());
        }

        // Load TestBundledMain through the child URLClassLoader
        System.out.println("[TEST] Loading net.minecraft.server.TestBundledMain through child URLClassLoader...");
        Class<?> bundledClass = Class.forName("net.minecraft.server.TestBundledMain", true, gameClassLoader);

        Method runMethod = bundledClass.getMethod("run");
        runMethod.invoke(null);

        // Verify that the captured classloader is the child URLClassLoader!
        if (!RedoxBridge.isClassLoaderCaptured()) {
            throw new AssertionError("FAIL: RedoxBridge.isClassLoaderCaptured() returned false after loading bundled class!");
        }

        ClassLoader captured = RedoxBridge.getCapturedClassLoader();
        System.out.println("[TEST] Captured ClassLoader = " + captured);

        if (captured != gameClassLoader) {
            throw new AssertionError("FAIL: Captured classloader (" + captured + ") does not match game URLClassLoader (" + gameClassLoader + ")!");
        }
        System.out.println("[TEST] PASS: The captured classloader is indeed the child URLClassLoader, NOT the AppClassLoader.");

        // Define game bridge classes into this captured URLClassLoader
        RedoxBridge.ensureInitialized();

        Class<?> baseClass = Class.forName("net.redoxmc.bridge.game.RedoxGameBridgeBase", true, captured);
        Class<?> childClass = Class.forName("net.redoxmc.bridge.game.RedoxGameBridge", true, captured);

        if (!baseClass.isAssignableFrom(childClass)) {
            throw new AssertionError("FAIL: In inheritance check: RedoxGameBridge does not inherit from RedoxGameBridgeBase!");
        }

        System.out.println("[TEST] PASS: Bridge-game classes defined into child URLClassLoader with full inheritance.");

        // Test DefineClass failure in child classloader
        byte[] invalidClass = new byte[] { 1, 2, 3, 4 };
        boolean res = RedoxBridge.testDefineClass("corrupt.CorruptedBundledClass", invalidClass);
        if (res) {
            throw new AssertionError("FAIL: Corrupt class unexpectedly loaded!");
        }
        System.out.println("[TEST] PASS: Corrupt class definition in child classloader failed gracefully without crashing.");

        System.out.println("[TEST] ========================================================");
        System.out.println("[TEST] BUNDLER SIMULATION TEST PASSED SUCCESSFULLY!");
        System.out.println("[TEST] ========================================================");
    }
}

