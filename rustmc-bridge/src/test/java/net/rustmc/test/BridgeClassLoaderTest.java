package net.rustmc.test;

import net.rustmc.bridge.RustBridge;
import java.lang.reflect.Method;

public class BridgeClassLoaderTest {
    public static void main(String[] args) throws Exception {
        System.out.println("[TEST] Starting BridgeClassLoaderTest for Task 1.2...");

        // 1. Verify classloader capture when first net/minecraft/* class loads
        System.out.println("[TEST] Step 1: Loading net.minecraft.client.TestClientEntry...");
        Class<?> clientClass = Class.forName("net.minecraft.client.TestClientEntry");
        ClassLoader clientLoader = clientClass.getClassLoader();
        System.out.println("[TEST] clientLoader = " + clientLoader);

        if (!RustBridge.isClassLoaderCaptured()) {
            throw new AssertionError("FAIL: RustBridge.isClassLoaderCaptured() returned false after loading net.minecraft class!");
        }

        ClassLoader capturedLoader = RustBridge.getCapturedClassLoader();
        System.out.println("[TEST] capturedLoader = " + capturedLoader);
        if (capturedLoader == null) {
            throw new AssertionError("FAIL: RustBridge.getCapturedClassLoader() returned null!");
        }

        if (capturedLoader != clientLoader) {
            throw new AssertionError("FAIL: Captured classloader does not match TestClientEntry classloader!");
        }
        System.out.println("[TEST] PASS: ClassLoader successfully captured from first net/minecraft/* class.");

        // 2. Verify that client and server in the same game environment share the same captured classloader
        System.out.println("[TEST] Step 2: Loading net.minecraft.server.TestServerEntry...");
        Class<?> serverClass = Class.forName("net.minecraft.server.TestServerEntry");
        ClassLoader serverLoader = serverClass.getClassLoader();
        System.out.println("[TEST] serverLoader = " + serverLoader);

        if (capturedLoader != serverLoader) {
            throw new AssertionError("FAIL: Captured classloader differs between client and server classes!");
        }
        System.out.println("[TEST] PASS: Captured classloader reference is identical for client and server classes.");

        // 3. Verify DefineClass outside hook defines superclass before subclass
        System.out.println("[TEST] Step 3: Triggering RustBridge.ensureInitialized() to define game bridge classes...");
        RustBridge.ensureInitialized();

        Class<?> baseClass = Class.forName("net.rustmc.bridge.game.RustGameBridgeBase", true, capturedLoader);
        Class<?> childClass = Class.forName("net.rustmc.bridge.game.RustGameBridge", true, capturedLoader);

        if (!baseClass.isAssignableFrom(childClass)) {
            throw new AssertionError("FAIL: RustGameBridge does not inherit from RustGameBridgeBase!");
        }

        Object bridgeInstance = childClass.getDeclaredConstructor().newInstance();
        Method getModuleName = baseClass.getMethod("getModuleName");
        String moduleName = (String) getModuleName.invoke(bridgeInstance);
        if (!"rustmc-bridge-game".equals(moduleName)) {
            throw new AssertionError("FAIL: Unexpected module name: " + moduleName);
        }

        Method getVersion = childClass.getMethod("getVersion");
        String version = (String) getVersion.invoke(null);
        System.out.println("[TEST] Instantiated RustGameBridge successfully: module=" + moduleName + ", version=" + version);
        System.out.println("[TEST] PASS: Superclass and subclass defined successfully with proper inheritance.");

        // 4. Verify DefineClass failure resilience (logs class name, clears exception, does NOT crash JVM)
        System.out.println("[TEST] Step 4: Testing DefineClass failure resilience with corrupt bytecode...");
        byte[] corruptBytecode = new byte[] { (byte) 0xDE, (byte) 0xAD, (byte) 0xBE, (byte) 0xEF, 0, 1, 2, 3 };
        boolean defineResult = RustBridge.testDefineClass("corrupt.CorruptedClass", corruptBytecode);
        if (defineResult) {
            throw new AssertionError("FAIL: testDefineClass unexpectedly succeeded for corrupt bytecode!");
        }

        // Verify JVM is still fully operational
        System.out.println("[TEST] PASS: Corrupt class definition failed gracefully without crashing the JVM.");

        System.out.println("[TEST] ========================================================");
        System.out.println("[TEST] ALL CHECKS PASSED SUCCESSFULLY FOR TASK 1.2!");
        System.out.println("[TEST] ========================================================");
    }
}
