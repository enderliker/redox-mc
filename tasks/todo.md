# Lista de Tareas: Modloader Nativo en Rust para Minecraft 26.3

## Fase 1: Fundación y Slice Vertical Mínimo

### Task 1.1: Agente JVMTI Base en Rust
**Description:** Crear el crate `redox-mc-loader` configurado como `cdylib`, implementando el punto de entrada estándar `Agent_OnLoad` de JVMTI. Inicializar el entorno, guardar los punteros a `JavaVM` y `jvmtiEnv`, activar capacidades (`can_generate_all_class_hook_events`) y registrar el callback `ClassFileLoadHook`. Usar las funciones canónicas de JVMTI `AddToBootstrapClassLoaderSearch` y `AddToSystemClassLoaderSearch`.

**Acceptance criteria:**
- [x] Compila como `libredox_mc_loader.so` sin errores ni advertencias.
- [x] `java -agentpath:libredox_mc_loader.so -version` inicializa el agente, imprime el banner informativo y finaliza con código 0.
- [x] La limpieza en `VMDeath` libera recursos sin producir fallos de segmentación.

**Verification:**
- [x] Build succeeds: `cargo build -p redox-mc-loader`
- [x] Manual check: Ejecutar `java -agentpath:target/debug/libredox_mc_loader.so -version` en la terminal.

**Dependencies:** None

**Files likely touched:**
- `redox-mc-loader/Cargo.toml`
- `redox-mc-loader/src/lib.rs`
- `redox-mc-loader/src/jvmti.rs`

**Estimated scope:** Medium | **Effort:** M (2-3 días)

---

### Task 1.2: Arquitectura del Puente Java y Manejo del Bundler
**Description:** Implementar la arquitectura del puente Java dividida en tres módulos: `redox-mc-bridge-core` (cero dependencias de Minecraft, contiene `RedoxBridge` y se añade con `AddToBootstrapClassLoaderSearch`), `redox-mc-bridge-game` (extiende clases comunes de Minecraft) y `redox-mc-bridge-client` (reservado para el cliente). Para resolver el aislamiento del `URLClassLoader` del bundler, capturar el `jobject loader` recibido en `ClassFileLoadHook` cuando se carga la primera clase en `net/minecraft/*` (ignorando el bundler), guardarlo como `GlobalReference` y, fuera del hook (en la primera llamada del puente), utilizar JNI `DefineClass` sobre ese classloader para definir las clases de `bridge-game` (superclases primero).

**Acceptance criteria:**
- [x] `redox-mc-bridge-core` se compila sin dependencias del jar de Minecraft y es visible globalmente en la JVM tras `AddToBootstrapClassLoaderSearch`.
- [x] En `ClassFileLoadHook`, la intercepción de la primera clase `net/minecraft/*` captura y preserva la referencia global al classloader del juego.
- [x] Fuera del hook, `DefineClass` define con éxito `RedoxGameBridgeBase` y `RedoxGameBridge` en el classloader del juego, resolviendo superclases primero. Si `DefineClass` falla, el error se registra con el nombre de la clase, se limpia la excepción y no tumba la JVM.
- [x] `bridge-client` queda aislado, evitando que el servidor dedicado cargue tipos gráficos.

**Verification:**
- [x] Build succeeds: Generar los sub-jars (`bridge-core.jar`, `bridge-game.jar`, `bridge-client.jar`).
- [x] Tests pass: Pruebas de integración (`BridgeClassLoaderTest`, `BundlerSimulationTest` y ejecución de `/home/ender/server.jar --help`) validando captura en cliente y servidor, resolución de tipos base y tolerancia a fallos en `DefineClass`.

**Dependencies:** Task 1.1

**Files likely touched:**
- `redox-mc-bridge/bridge-core/pom.xml`
- `redox-mc-bridge/bridge-core/src/main/java/net/redoxmc/bridge/RedoxBridge.java`
- `redox-mc-bridge/bridge-game/pom.xml`
- `redox-mc-bridge/bridge-client/pom.xml`
- `redox-mc-loader/src/jni.rs`
- `redox-mc-loader/src/class_injector.rs`

**Estimated scope:** Large | **Effort:** L (3-5 días)

---

### Task 1.3: Crate Adaptador de Versión (`redox-mc-adapter-26-3`) y Tabla de Hooks
**Description:** Crear el crate `redox-mc-adapter-26-3` que centraliza las constantes, firmas de métodos y descriptores específicos de Minecraft 26.3. Definir la tabla canónica de hooks (`HookTarget { class_name, method_name, method_desc }`) que servirá de entrada a las transformaciones de bytecode en el loader, desacoplando los nombres del motor.

**Acceptance criteria:**
- [x] El crate expone la tabla de hooks para `Bootstrap.bootStrap()` (`()V`), `BuiltInRegistries.freeze()` (`()V`), `MinecraftServer.tickChildren` (`(Ljava/util/function/BooleanSupplier;)V`) y `ServerGamePacketListenerImpl.handleChat` (`(Lnet/minecraft/network/protocol/game/ServerboundChatPacket;)V`).
- [x] Métodos y traits en `redox-mc-adapter-api` para consultar firmas y nombres de clases específicas de 26.3 sin exponerlas al loader principal.
- [x] Pruebas unitarias e integración en Rust validan que los descriptores JNI corresponden a la especificación de JVM y coinciden exactamente con el bytecode de los jars oficiales.

**Verification:**
- [x] Tests pass: `cargo test -p redox-mc-adapter-26-3`
- [x] Manual check: Detección en runtime de hooks durante el arranque del `server.jar` real (`server.jar --initSettings`).

**Dependencies:** Task 1.1

**Files likely touched:**
- `redox-mc-adapter-26-3/Cargo.toml`
- `redox-mc-adapter-26-3/src/lib.rs`
- `redox-mc-adapter-26-3/src/hooks.rs`

**Estimated scope:** Medium | **Effort:** M (2-3 días)

---

### Task 1.4: Transformador de Bytecode en Rust con Método Envoltorio
**Description:** Realizar un spike de investigación sobre crates de Rust para parsing y escritura de classfiles Java 25 (formato 69.0). Implementar en Rust dentro de `ClassFileLoadHook` la técnica del "método envoltorio": renombrar el método original definido en `redox-mc-adapter-26-3` (ej. `bootStrap()`) a `bootStrap$original()` y generar un nuevo método con el nombre y firma original que llame a `RedoxBridge.onBootstrapEntry()` y luego a `bootStrap$original()`, evitando la necesidad de recalcular StackMapTables.

**Acceptance criteria:**
- [x] Se valida un crate de Rust (`ristretto_classfile = "0.34.0"`) capaz de leer y escribir classfiles versión 69.0 sin corromper el pool de constantes.
- [x] La transformación se ejecuta íntegramente en código Rust dentro de `ClassFileLoadHook` sin invocar clases Java intermedias.
- [x] Al cargar `Bootstrap`, el método envoltorio se genera válidamente y se ejecuta antes que la lógica de Minecraft sin provocar `VerifyError`.

**Verification:**
- [x] Tests pass: Tests unitarios e integración en Rust (`redox-mc-loader/tests/test_wrapper_transformer.rs`) validando parsing de descriptores, transformación de `Bootstrap.class` real y clase personalizada con validación estructural vía `javap -v -p`.
- [x] Manual check: Ejecución del `server.jar` real (`--initSettings`) con el agente confirmando intercepción y ejecución de `Bootstrap.bootStrap()` y `BuiltInRegistries.freeze()` sin `VerifyError`.

**Dependencies:** Task 1.2, Task 1.3

**Files likely touched:**
- `redox-mc-loader/Cargo.toml`
- `redox-mc-loader/src/transformer/mod.rs`
- `redox-mc-loader/src/transformer/wrapper.rs`
- `redox-mc-loader/src/transformer/classfile.rs`

**Estimated scope:** Large | **Effort:** L (3-5 días)

---

### Task 1.5: Manifiesto de Mod y Orden de Carga Determinista
**Description:** Definir el formato del manifiesto de mods (`mod.toml`: id, version, side, dependencies, min_loader_version). Implementar en `redox-mc-loader` el parser y el algoritmo de ordenación determinista: validación de compatibilidad con el entorno (ignorar mods cliente en servidor dedicado), resolución topológica de dependencias y ordenación lexicográfica por mod ID en caso de nodos independientes.

**Acceptance criteria:**
- [x] Parser de `mod.toml` robusto ante campos desconocidos y con validación de tipos semver.
- [x] La resolución de dependencias detecta ciclos y dependencias faltantes abortando con un error legible.
- [x] El orden de los mods resultantes es estrictamente determinista entre diferentes ejecuciones y plataformas.

**Verification:**
- [x] Tests pass: `cargo test -p redox-mc-loader --test manifest_resolution` con 10 pruebas cubriendo grafos topológicos, detección de ciclos, filtrado por entorno (cliente/servidor) y desempate lexicográfico.

**Dependencies:** Task 1.2, Task 1.4

**Files likely touched:**
- `redox-mc-loader/src/manifest.rs`
- `redox-mc-loader/src/mod_resolver.rs`

**Estimated scope:** Medium | **Effort:** M (2-3 días)

---

### Task 1.6: Cargador Dinámico de Mods y Slice Vertical Completo
**Description:** Implementar en `redox-mc-loader` el descubrimiento de binarios `.so`/`.dll` en `mods/`, asociándolos a su `mod.toml`. Cargar cada biblioteca mediante `libloading`, resolver el símbolo `redox_mod_init` respetando el orden determinista, y verificar la ejecución completa con un mod de ejemplo `examples/test-mod` que se suscriba al hook de bootstrap.

**Acceptance criteria:**
- [ ] `examples/test-mod` compila como `libtest_mod.so` conteniendo su `mod.toml`.
- [ ] El loader detecta el mod, valida su compatibilidad y lo carga en el orden determinista establecido.
- [ ] Al arrancar el servidor dedicado con `java -agentpath`, el hook de bootstrap se dispara y el código Rust del mod se ejecuta con éxito en el proceso.

**Verification:**
- [ ] Build succeeds: `cargo build -p test-mod`
- [ ] Manual check: Arrancar `server.jar` con el mod en `mods/` y confirmar en los logs la salida del mod antes de que el servidor empiece a escuchar conexiones.

**Dependencies:** Task 1.5

**Files likely touched:**
- `redox-mc-loader/src/mod_loader.rs`
- `examples/test-mod/Cargo.toml`
- `examples/test-mod/mod.toml`
- `examples/test-mod/src/lib.rs`

**Estimated scope:** Medium | **Effort:** M (2-3 días)

---

## Checkpoint: Slice Vertical Funcional
- [ ] La JVM arranca con `-agentpath:libredox_mc_loader.so` contra `server.jar` con el bundler activo.
- [ ] `redox-mc-bridge-core` se resuelve en el bootstrap search y el hook de `Bootstrap.bootStrap()` funciona sin `VerifyError`.
- [ ] `test-mod` se carga en orden determinista y ejecuta código Rust dentro del proceso de Minecraft.
- [ ] El servidor finaliza el arranque con éxito.

---

## Fase 2: Arquitectura ABI, Aislamiento y Generador de Bindings

### Task 2.1: Capa `redox-mc-abi` y Ownership de Memoria
**Description:** Crear el crate `redox-mc-abi` estableciendo tipos `#[repr(C)]`, vtables y punteros opacos. Implementar la regla estricta de deasignación simétrica: la memoria asignada en el loader se libera en el loader (`redox_loader_free_*`), y la asignada en el mod se libera en el mod (`redox_mod_free_*`), prohibiendo el cruce de allocators entre diferentes binarios dinámicos.

**Acceptance criteria:**
- [ ] Toda estructura que cruza la frontera de la biblioteca dinámica implementa `#[repr(C)]`.
- [ ] Funciones de deasignación explícitas expuestas y documentadas para buffers y cadenas.
- [ ] Pruebas unitarias de layout y alineación de memoria garantizan estabilidad binaria.

**Verification:**
- [ ] Tests pass: `cargo test -p redox-mc-abi`
- [ ] Build succeeds: `cargo check -p redox-mc-abi --all-targets`

**Dependencies:** Task 1.6

**Files likely touched:**
- `redox-mc-abi/Cargo.toml`
- `redox-mc-abi/src/lib.rs`
- `redox-mc-abi/src/types.rs`
- `redox-mc-abi/src/vtable.rs`
- `redox-mc-abi/src/memory.rs`

**Estimated scope:** Medium | **Effort:** M (2-3 días)

---

### Task 2.2: Generador de Bindings desde Classfiles (`redox-mc-bindgen`)
**Description:** Crear una herramienta CLI en Rust que lea archivos `.class` directamente desde los archivos `.jar` de Minecraft (e.g. `server-inner/server-26.3.jar` o librerías) y genere bindings en Rust tipados con firmas y descriptores JNI automáticos para tipos comunes (`Identifier`, `ResourceKey`, `BlockPos`, `Direction`).

**Acceptance criteria:**
- [ ] Parsea `.class` empaquetados en `.jar` extrayendo clases, interfaces, firmas de métodos y descriptores de campos.
- [ ] Emite código Rust sintácticamente correcto que encapsula las llamadas JNI y cachea `jmethodID` y `jfieldID`.
- [ ] No requiere código fuente Java decompilado para funcionar.

**Verification:**
- [ ] Build succeeds: `cargo build -p redox-mc-bindgen`
- [ ] Tests pass: `cargo run -p redox-mc-bindgen -- --jar /home/ender/server-inner/server-26.3.jar --class net/minecraft/resources/Identifier` y compilar el archivo emitido con `cargo check`.

**Dependencies:** Task 2.1

**Files likely touched:**
- `redox-mc-bindgen/Cargo.toml`
- `redox-mc-bindgen/src/main.rs`
- `redox-mc-bindgen/src/jar_reader.rs`
- `redox-mc-bindgen/src/emitter.rs`

**Estimated scope:** Medium | **Effort:** M (2-3 días)

---

### Task 2.3: Macro de Exportación y Aislamiento de Pánicos en el Mod
**Description:** Implementar en `redox-mc-api` la macro procedural o declarativa `#[redox_export]`. Esta macro envuelve el cuerpo de cada función de mod exportada en `std::panic::catch_unwind(AssertUnwindSafe(...))` **dentro del propio `cdylib` del mod**. Si ocurre un pánico, se registra el mensaje y el backtrace nativo en el log, y se retorna un código de error de la ABI (`FfiResult::Panic`) en lugar de abortar el proceso al cruzar la frontera FFI.

**Acceptance criteria:**
- [ ] El pánico es capturado dentro del runtime `std` del mod antes de salir por la frontera `extern "C"`.
- [ ] Un mod de prueba con `panic!("error de prueba")` no tumba la JVM ni produce `SIGABRT`.
- [ ] No se registran signal handlers para `SIGSEGV` ni Access Violation en el agente nativo, preservando el comportamiento de HotSpot.

**Verification:**
- [ ] Tests pass: Test de integración con una función exportada que entra en pánico y valida el retorno de `FfiResult::Panic`.

**Dependencies:** Task 2.1

**Files likely touched:**
- `redox-mc-api/src/macros.rs`
- `redox-mc-loader/src/safety.rs`

**Estimated scope:** Medium | **Effort:** M (2-3 días)

---

### Task 2.4: Microbenchmark de Cruces JNI y Frontera de Rendimiento
**Description:** Implementar un harness de microbenchmark en `redox-mc-loader` para medir con precisión la sobrecarga real de llamadas nativas JNI en la máquina local (Java 25 en Linux x86_64). Medir coste de: llamadas sin argumentos, paso de primitivos, strings pequeños y objetos complejos, validando la directriz de diseño declarativo.

**Acceptance criteria:**
- [ ] Ejecuta una suite reproducible de benchmarks registrando latencia promedio y percentiles (p50, p99).
- [ ] Genera un informe documentando la sobrecarga real (contrastando con la referencia aproximada de 10-25 ns).

**Verification:**
- [ ] Manual check: `cargo run -p redox-mc-loader --bin jni-bench` imprime los resultados estadísticos de latencia.

**Dependencies:** Task 2.1

**Files likely touched:**
- `redox-mc-loader/src/bench/mod.rs`
- `redox-mc-loader/src/bench/main.rs`

**Estimated scope:** Small | **Effort:** S (1-2 días)

---

## Checkpoint: Frontera Binaria y Seguridad
- [ ] Tipos `repr(C)` y reglas de deasignación simétrica verificadas.
- [ ] Bindgen operativo extrayendo metadatos directamente de `.class` en jars.
- [ ] Pánicos en mods capturados localmente y aislados sin abortar el proceso ni sustituir manejadores de señales de HotSpot.
- [ ] Sobrecarga JNI caracterizada y medida.

---

## Fase 3: Ciclo de Vida y Sistema de Eventos Simples

### Task 3.1: Hook de Registro en `BuiltInRegistries.freeze()`
**Description:** En el callback `ClassFileLoadHook` de `redox-mc-loader`, transformar `net.minecraft.core.registries.BuiltInRegistries` aplicando la técnica de método envoltorio sobre `private static void freeze()`, utilizando la definición de hook de `redox-mc-adapter-26-3`. Renombrar `freeze()` a `freeze$original()` y generar un wrapper que invoque `RedoxBridge.onRegisterContent()` inmediatamente antes de `freeze$original()`.

**Acceptance criteria:**
- [ ] El hook se dispara exactamente cuando todos los registros vanilla ya contienen sus elementos iniciales creados por `createContents()`.
- [ ] Durante la ejecución de `RedoxBridge.onRegisterContent()`, los registros tienen `isFrozen() == false`.
- [ ] Inmediatamente después del hook, `freeze$original()` sella todos los registros (`isFrozen() == true`).
- [ ] No se produce corrupción de stack map frames ni errores de verificación de bytecode.

**Verification:**
- [ ] Manual check: Verificar en el log del servidor que el hook reporta que los registros aún están abiertos y que posteriormente se sellan.

**Dependencies:** Task 1.3, Task 1.4, Task 2.3

**Files likely touched:**
- `redox-mc-loader/src/transformer/registry_hook.rs`
- `redox-mc-bridge/bridge-core/src/main/java/net/redoxmc/bridge/RedoxBridge.java`

**Estimated scope:** Medium | **Effort:** M (2-3 días)

---

### Task 3.2: Motor de Eventos Tipado en Rust (`redox-mc-api`)
**Description:** Implementar en `redox-mc-api` un bus de eventos pub/sub tipado en Rust con prioridades de ejecución (`HIGHEST`, `NORMAL`, `LOWEST`) y soporte de cancelación (`EventResult::Allow`, `EventResult::Cancel`).

**Acceptance criteria:**
- [ ] Suscripción de callbacks con funciones o closures tipadas.
- [ ] Orden estricto de despacho según prioridad.
- [ ] Posibilidad de abortar eventos cancelables deteniendo la propagación hacia handlers de menor prioridad.

**Verification:**
- [ ] Tests pass: `cargo test -p redox-mc-api --test event_bus`

**Dependencies:** Task 2.1

**Files likely touched:**
- `redox-mc-api/src/event/mod.rs`
- `redox-mc-api/src/event/bus.rs`

**Estimated scope:** Small | **Effort:** S (1-2 días)

---

### Task 3.3: Inyección de Eventos Simples (Server Tick y Chat)
**Description:** Inyectar hooks envoltorios en `MinecraftServer.tickChildren()` para disparar `ServerTickEvent` y en `ServerGamePacketListenerImpl.handleChat()` para `ServerChatEvent`. Conectar el puente Java con el bus de eventos de Rust y verificar en un servidor headless sin mocks.

**Acceptance criteria:**
- [ ] `ServerTickEvent` se despacha cada 50 ms (20 Hz) transmitiendo el número de tick sin fugas de memoria.
- [ ] `ServerChatEvent` intercepta los mensajes de chat; si el mod cancela el evento, el mensaje no se entrega a los destinatarios.
- [ ] Las pruebas de verificación se ejecutan directamente contra el servidor headless en lugar de usar mocks de Java.

**Verification:**
- [ ] Manual check: Mod de prueba que registra un contador de ticks y bloquea mensajes de chat con prefijo `!`, probado en el servidor dedicado.

**Dependencies:** Task 3.1, Task 3.2

**Files likely touched:**
- `redox-mc-bridge/bridge-game/src/main/java/net/redoxmc/bridge/event/ServerEvents.java`
- `redox-mc-loader/src/events.rs`
- `examples/test-mod/src/lib.rs`

**Estimated scope:** Medium | **Effort:** M (2-3 días)

---

## Checkpoint: Ciclo de Vida y Eventos
- [ ] Hook de registro en `BuiltInRegistries.freeze()` verificado y seguro.
- [ ] Eventos de tick y chat despachados a Rust en servidor headless real.
- [ ] Cero fugas de referencias JNI locales en bucles de alta frecuencia.

---

## Fase 4: Registro Declarativo de Ítems y Soporte Windows

### Task 4.1: Clase Puente `GenericRustItem`
**Description:** Implementar en `redox-mc-bridge-game` la clase `GenericRustItem extends Item`. Esta clase se compila contra el jar del juego y se inyecta en su classloader mediante `DefineClass`. Delega interacciones (`use`, `useOn`, `finishUsingItem`) a callbacks nativos en `RedoxBridge` sólo si el mod declaró comportamiento activo; si el ítem es pasivo, el cruce JNI es omitido.

**Acceptance criteria:**
- [ ] Se instancia correctamente pasando `Item.Properties.setId(id)`.
- [ ] Los métodos virtuales invocan el callback nativo sólo si el mod tiene lógica registrada.
- [ ] La clase reside en `bridge-game` y es accesible para el motor del juego sin conflictos de visibilidad.

**Verification:**
- [ ] Build succeeds: Compilar `redox-mc-bridge-game.jar`.

**Dependencies:** Task 1.2, Task 3.1

**Files likely touched:**
- `redox-mc-bridge/bridge-game/src/main/java/net/redoxmc/bridge/item/GenericRustItem.java`

**Estimated scope:** Medium | **Effort:** M (2-3 días)

---

### Task 4.2: Declaración de Ítems y Hash de Registros
**Description:** Implementar en `redox-mc-api` el builder declarativo `ItemDefinition` y en `redox-mc-loader` el registro determinista en `BuiltInRegistries.ITEM`. Implementar el generador de hash de contenido de registros (SHA-256 o xxHash sobre los IDs y propiedades de ítems registrados en orden determinista).

**Acceptance criteria:**
- [ ] Sintaxis declarativa: `ItemDefinition::new("testmod:ruby").max_stack_size(64)`.
- [ ] El registro se ejecuta en el orden determinista dictado por la Fase 1.
- [ ] Se computa un hash acumulado de todos los ítems registrados disponible para la fase de handshake.

**Verification:**
- [ ] Tests pass: `cargo test -p redox-mc-loader --test registry_hash` confirmando hash idéntico en ejecuciones sucesivas.

**Dependencies:** Task 4.1, Task 1.5

**Files likely touched:**
- `redox-mc-api/src/item.rs`
- `redox-mc-loader/src/registry/item.rs`
- `redox-mc-loader/src/registry/hash.rs`

**Estimated scope:** Medium | **Effort:** M (2-3 días)

---

### Task 4.3: Validación de Ítems en Servidor Headless
**Description:** Registrar el ítem `testmod:ruby` en `examples/test-mod` y validar su comportamiento en un servidor headless real mediante comandos de consola.

**Acceptance criteria:**
- [ ] El servidor arranca sin advertencias de ítems no registrados.
- [ ] El comando `/give @p testmod:ruby 5` entrega el ítem al inventario del jugador de prueba.
- [ ] La interacción con el ítem dispara el callback en Rust si está configurado.

**Verification:**
- [ ] Manual check: Ejecutar `/give @p testmod:ruby 1` en la consola del servidor headless y confirmar el registro en el log.

**Dependencies:** Task 4.2

**Files likely touched:**
- `examples/test-mod/src/lib.rs`

**Estimated scope:** Small | **Effort:** S (1-2 días)

---

### Task 4.4: Soporte y Compilación Cruzada para Windows x86_64 (`.dll`)
**Description:** Configurar el soporte de compilación para la plataforma Windows x86_64 (`x86_64-pc-windows-msvc` / `gnu`) tanto para el loader (`redox_mc_loader.dll`) como para los mods (`.dll`). Garantizar que la capa `redox-mc-abi`, los shims JNI y el linker generen binarios nativos válidos para Windows antes de implementar los bloques y assets (Tarea 5.3) y el hot-reload (Fase 7).

**Acceptance criteria:**
- [ ] `redox-mc-loader` y `test-mod` compilan exitosamente para `x86_64-pc-windows-msvc` (o mingw-w64).
- [ ] Los símbolos de exportación JVMTI (`Agent_OnLoad`) y JNI se exportan correctamente sin decoraciones de nombre incorrectas.
- [ ] `redox-mc-abi` valida layouts de tipos compatibles con la ABI de Windows de 64 bits.

**Verification:**
- [ ] Build succeeds: `cargo build --target x86_64-pc-windows-gnu -p redox-mc-loader -p test-mod` (o cross/cargo clippy para Windows).

**Dependencies:** Task 2.1

**Files likely touched:**
- `redox-mc-loader/build.rs`
- `redox-mc-loader/src/lib.rs`
- `redox-mc-abi/src/lib.rs`

**Estimated scope:** Medium | **Effort:** M (2-3 días)

---

## Fase 5: Recursos Virtuales y Bloques con Assets

### Task 5.1: Inyector de Paquetes Virtuales (`VirtualModPackResources`)
**Description:** Implementar en `redox-mc-bridge-game` la clase `VirtualModPackResources implements PackResources` (compatible con formatos 97 y 121 de MC 26.3). Interceptar mediante transformación en Rust la creación de `PackRepository` para registrar un `RepositorySource` virtual que sirva streams de memoria consultando a `redox-mc-loader` vía JNI.

**Acceptance criteria:**
- [ ] `PackRepository.reload()` descubre el pack virtual tanto en cliente (`CLIENT_RESOURCES`) como en servidor (`SERVER_DATA`).
- [ ] `VirtualModPackResources.getResource(...)` entrega `InputStream` para archivos virtuales (`pack.mcmeta`, blockstates, models, items, loot tables, tags) generados en memoria desde Rust.
- [ ] No requiere escribir ningún archivo temporal al disco.

**Verification:**
- [ ] Tests pass: Prueba en servidor headless solicitando un recurso virtual (loot table o receta) mediante `ResourceManager`.

**Dependencies:** Task 3.1, Task 1.2

**Files likely touched:**
- `redox-mc-bridge/bridge-game/src/main/java/net/redoxmc/bridge/pack/VirtualModPackResources.java`
- `redox-mc-bridge/bridge-game/src/main/java/net/redoxmc/bridge/pack/ModRepositorySource.java`
- `redox-mc-loader/src/packs.rs`

**Estimated scope:** Large | **Effort:** L (3-5 días)

---

### Task 5.2: Clase Puente `GenericRedoxBlock` y Sincronización de Estados
**Description:** Implementar en `redox-mc-bridge-game` la clase `GenericRedoxBlock extends Block`. Al registrar el bloque:
1. Asignar `properties.setId(id)`.
2. Registrar cada `BlockState` en [`Block.BLOCK_STATE_REGISTRY`](file:///home/ender/rustmc/out-client/net/minecraft/world/level/block/Block.java#L81) e invocar `state.initCache()`.
3. Crear el `BlockItem` asociado en `BuiltInRegistries.ITEM` y vincularlo en `Item.BY_BLOCK`.

**Acceptance criteria:**
- [ ] El bloque se registra en `BuiltInRegistries.BLOCK` y sus estados en la paleta global `BLOCK_STATE_REGISTRY`.
- [ ] `Block.getId(state)` retorna un ID mayor que cero.
- [ ] El `BlockItem` permite colocar y romper el bloque en el mundo.

**Verification:**
- [ ] Manual check: Colocar el bloque en el mundo mediante comando `/setblock ~ ~ ~ testmod:ruby_block` en servidor headless.

**Dependencies:** Task 5.1, Task 4.2

**Files likely touched:**
- `redox-mc-bridge/bridge-game/src/main/java/net/redoxmc/bridge/block/GenericRedoxBlock.java`
- `redox-mc-loader/src/registry/block.rs`

**Estimated scope:** Large | **Effort:** L (3-4 días)

---

### Task 5.3: API Declarativa de Bloques y Assets en Rust (con Soporte Windows)
**Description:** Implementar en `redox-mc-api` la declaración de bloques con autogeneración de assets virtuales (`blockstates/<id>.json`, `models/block/<id>.json`, `items/<id>.json` y `loot_table/blocks/<id>.json`). Incorporar las propiedades y estados del bloque al cálculo del hash de contenido determinista. Verificar que los assets se generan y sirven correctamente tanto en Linux como en Windows (preparado en Tarea 4.4).

**Acceptance criteria:**
- [ ] Declaración limpia en Rust: `BlockDefinition::new("testmod:ruby_block").strength(3.0).cube_model("testmod:block/ruby_block")`.
- [ ] El pack virtual entrega los JSON correspondientes al cliente sin texturas de error ni modelos missing.
- [ ] Romper el bloque suelta el ítem generado en el loot table virtual.

**Verification:**
- [ ] Manual check: Romper el bloque en cliente/servidor y verificar la obtención del ítem.

**Dependencies:** Task 5.2, Task 4.4

**Files likely touched:**
- `redox-mc-api/src/block.rs`
- `redox-mc-api/src/assets.rs`
- `examples/test-mod/src/lib.rs`

**Estimated scope:** Medium | **Effort:** M (2-3 días)

---

## Checkpoint: Bloques e Ítems Completos
- [ ] Paquetes virtuales operando en memoria sin disco.
- [ ] Bloque registrado con `BlockState` en la paleta global y `BlockItem` vinculado.
- [ ] Hash de contenido actualizado con bloques y sus estados.
- [ ] Soporte de compilación en Windows validado.

---

## Fase 6: Canales de Red y Handshake de Configuración

### Task 6.1: Decodificador Universal de Payloads de Mod
**Description:** Transformar `CustomPacketPayload` en Rust para que los identificadores de canales de mods no caigan en `DiscardedPayload`, sino en `ModRawPayload(Identifier id, byte[] data)` dentro de `redox-mc-bridge-game`, reteniendo los bytes originales y despachándolos a la capa nativa.

**Acceptance criteria:**
- [ ] Paquetes con namespaces de mods retienen el búfer binario completo.
- [ ] Se transfieren a Rust como slice nativo `&[u8]`.
- [ ] No altera el comportamiento de payloads vanilla (`BrandPayload`).

**Verification:**
- [ ] Tests pass: Prueba de codificación y decodificación de `ModRawPayload` con `FriendlyByteBuf`.

**Dependencies:** Task 3.1, Task 1.2

**Files likely touched:**
- `redox-mc-bridge/bridge-game/src/main/java/net/redoxmc/bridge/network/ModRawPayload.java`
- `redox-mc-loader/src/transformer/network_hook.rs`

**Estimated scope:** Small | **Effort:** S (1-2 días)

---

### Task 6.2: Handshake en Fase `CONFIGURATION` con Hash de Contenido
**Description:** Implementar `ModHandshakeConfigurationTask implements ConfigurationTask` en `redox-mc-bridge-game`. Interceptar `ServerConfigurationPacketListenerImpl.startConfiguration()` para encolar la tarea. El servidor envía la lista de mods y el hash de contenido de registros calculado en la Fase 4 y 5; si el cliente difiere en mods o en el hash de registros, el servidor desconecta al cliente con un mensaje de error detallado.

**Acceptance criteria:**
- [ ] Intercambio de paquetes de configuración antes de la fase `PLAY`.
- [ ] Si los mods y el hash de registros coinciden, la conexión progresa limpiamente a `PLAY`.
- [ ] Si un cliente tiene versiones incompatibles o diferente hash de registros (IDs de bloques desalineados), se desconecta mostrando el motivo exacto.

**Verification:**
- [ ] Manual check: Probar conexión con mod alterado intencionalmente y verificar pantalla de desconexión descriptiva.

**Dependencies:** Task 6.1, Task 4.2

**Files likely touched:**
- `redox-mc-bridge/bridge-game/src/main/java/net/redoxmc/bridge/network/ModHandshakeConfigurationTask.java`
- `redox-mc-loader/src/network/handshake.rs`

**Estimated scope:** Medium | **Effort:** M (2-3 días)

---

### Task 6.3: API de Canales de Red en Rust
**Description:** Exponer en `redox-mc-api` funciones para registrar canales y enviar/recibir mensajes tipados: `NetworkChannel::register("testmod:data").on_receive(...)`. Conectar el envío bidireccional mediante `ClientboundCustomPayloadPacket` y `ServerboundCustomPayloadPacket`.

**Acceptance criteria:**
- [ ] Envío fiable de mensajes serializados entre cliente y servidor.
- [ ] Los callbacks en Rust reciben el payload y la identidad del remitente.

**Verification:**
- [ ] Tests pass: Mod de ejemplo enviando un ping y validando la recepción del pong correspondiente.

**Dependencies:** Task 6.2

**Files likely touched:**
- `redox-mc-api/src/network.rs`
- `redox-mc-loader/src/network/mod.rs`

**Estimated scope:** Medium | **Effort:** M (2-3 días)

---

## Checkpoint: Red y Handshake
- [ ] Handshake en fase `CONFIGURATION` validando mods y hash de registros.
- [ ] Canales de red transmitiendo mensajes binarios personalizados sin corrupción.

---

## Fase 7: Hot-Reloading Seguro y Experiencia de Desarrollo

### Task 7.1: Hot-Reloading Seguro mediante Copia Temporal y Swap Atómico
**Description:** Implementar en `redox-mc-loader` el patrón de recarga segura: ante una recompilación del mod, copiar el binario a un archivo temporal (`mod_cache/mod_<timestamp>.so` o `.dll`), cargarlo con `libloading` y actualizar atómicamente la tabla de callbacks en memoria (`ArcSwap`). Omitir deliberadamente `dlclose` sobre la versión anterior para evitar crashes por destructores de thread-locals de Rust, closures colgantes y bloqueos de archivo en Windows (usando el soporte Windows de la Tarea 4.4).

**Acceptance criteria:**
- [ ] Recompilar un mod y ejecutar `/redox reload`: la nueva lógica toma efecto sin reiniciar la JVM ni desconectar clientes.
- [ ] No se producen crashes al llamar callbacks tras la recarga.
- [ ] En Windows, el binario del proyecto original no queda bloqueado por el proceso de la JVM.

**Verification:**
- [ ] Manual check: Modificar el mensaje de un evento de chat en vivo, recargar y confirmar el nuevo mensaje.

**Dependencies:** Task 6.3, Task 1.6, Task 4.4

**Files likely touched:**
- `redox-mc-loader/src/reload.rs`
- `redox-mc-loader/src/dispatcher.rs`

**Estimated scope:** Medium | **Effort:** M (2-3 días)

---

### Task 7.2: Herramienta CLI (`cargo-redox-mc`)
**Description:** Crear el binario `cargo-redox-mc` para automatizar el ciclo de desarrollo: `cargo redox-mc new <mod>`, `cargo redox-mc build` y `cargo redox-mc run` (con soporte para lanzar servidor headless rápido o cliente con el agente inyectado).

**Acceptance criteria:**
- [ ] Genera proyectos de mods con estructura estándar, `mod.toml` y configuración `cdylib`.
- [ ] `cargo redox-mc run --server` lanza el entorno de pruebas local en un solo comando.

**Verification:**
- [ ] Build succeeds: `cargo install --path cargo-redox-mc`
- [ ] Manual check: Crear y arrancar un mod nuevo utilizando exclusivamente `cargo-redox-mc`.

**Dependencies:** Task 7.1

**Files likely touched:**
- `cargo-redox-mc/Cargo.toml`
- `cargo-redox-mc/src/main.rs`
- `cargo-redox-mc/src/commands/build.rs`
- `cargo-redox-mc/src/commands/run.rs`

**Estimated scope:** Medium | **Effort:** M (2-3 días)

---

## Fase 8: Block Entities y Pantallas/GUIs

### Task 8.1: Clase Puente `GenericRedoxBlockEntity`
**Description:** Implementar en `redox-mc-bridge-game` la clase `GenericRedoxBlockEntity extends BlockEntity` y registrar su tipo en `BuiltInRegistries.BLOCK_ENTITY_TYPE`. Conectar la serialización y deserialización de NBT (`saveAdditional`, `loadAdditional`) y el ticker (`BlockEntityTicker`) con callbacks nativos en Rust.

**Acceptance criteria:**
- [ ] Los datos NBT escritos por el mod en Rust persisten tras guardar y recargar el mundo.
- [ ] El ticker del bloque se ejecuta periódicamente sólo si el mod lo declara activo.

**Verification:**
- [ ] Manual check: Colocar el bloque con estado, modificar datos, reiniciar el servidor headless y verificar que el estado se recupera intacto.

**Dependencies:** Task 5.2

**Files likely touched:**
- `redox-mc-bridge/bridge-game/src/main/java/net/redoxmc/bridge/block/GenericRedoxBlockEntity.java`
- `redox-mc-loader/src/registry/block_entity.rs`

**Estimated scope:** Medium | **Effort:** M (2-3 días)

---

### Task 8.2: Menús de Contenedor y Pantallas en Cliente
**Description:** Implementar `GenericRedoxMenu extends AbstractContainerMenu` en `redox-mc-bridge-game` y su tipo en `BuiltInRegistries.MENU`. Implementar la pantalla correspondiente en `redox-mc-bridge-client`. Conectar la apertura del menú y eventos de clics en slots con callbacks en Rust.

**Acceptance criteria:**
- [ ] Al interactuar con el bloque, se abre la interfaz con slots sincronizados entre cliente y servidor.
- [ ] La lógica del mod en Rust valida o rechaza el movimiento de ítems.
- [ ] Las clases de pantalla permanecen aisladas en `bridge-client`.

**Verification:**
- [ ] Manual check: Abrir interfaz gráfica en cliente y transferir ítems entre el contenedor y el jugador.

**Dependencies:** Task 8.1, Task 6.3

**Files likely touched:**
- `redox-mc-bridge/bridge-game/src/main/java/net/redoxmc/bridge/gui/GenericRedoxMenu.java`
- `redox-mc-bridge/bridge-client/src/main/java/net/redoxmc/bridge/gui/GenericRedoxScreen.java`
- `redox-mc-api/src/gui.rs`

**Estimated scope:** Large | **Effort:** L (3-5 días)

---

## Fase 9: Subsistema de Entidades (Servidor y Renderpearl)

### Task 9.1: Entidades en Servidor (Bridge, Atributos e IA)
**Description:** Implementar `GenericRedoxMob extends PathfinderMob` en `redox-mc-bridge-game`. Registrar en `BuiltInRegistries.ENTITY_TYPE`, inyectar sus atributos en `DefaultAttributes.SUPPLIERS` y reenviar `registerGoals()` a Rust para registrar metas en el `GoalSelector`.

**Acceptance criteria:**
- [ ] `DefaultAttributes.validate()` pasa sin error para la entidad registrada.
- [ ] La entidad puede ser invocada con `/summon testmod:custom_mob` en servidor headless.
- [ ] Ejecuta físicas, colisiones y pathfinding configurados desde Rust.

**Verification:**
- [ ] Manual check: Ejecutar `/summon testmod:custom_mob` en el servidor headless y verificar su comportamiento en consola.

**Dependencies:** Task 8.1, Task 3.1

**Files likely touched:**
- `redox-mc-bridge/bridge-game/src/main/java/net/redoxmc/bridge/entity/GenericRedoxMob.java`
- `redox-mc-loader/src/registry/entity.rs`

**Estimated scope:** Large | **Effort:** L (3-5 días)

---

### Task 9.2: Pipeline de Renderizado en Renderpearl para Cliente
**Description:** Implementar en `redox-mc-bridge-client` la clase `GenericRedoxEntityRenderer extends EntityRenderer` y registrarla en `EntityRenderers.PROVIDERS`. Implementar la extracción de estado en `createRenderState` y la sumisión de geometría cúbica/jerárquica en `submit()` mediante `SubmitNodeCollector` del nuevo subsistema gráfico Renderpearl de MC 26.3.

**Acceptance criteria:**
- [ ] `EntityRenderers.validateRegistrations()` confirma que la entidad tiene renderer registrado en el cliente.
- [ ] La entidad es visible en el cliente con modelo, textura y animaciones configuradas.
- [ ] No genera errores de shaders ni excepciones de Renderpearl en el bucle de render.

**Verification:**
- [ ] Manual check: Conectar cliente al servidor, spawnear la entidad y verificar renderizado fluido a 60 FPS.

**Dependencies:** Task 9.1, Task 5.1

**Files likely touched:**
- `redox-mc-bridge/bridge-client/src/main/java/net/redoxmc/bridge/entity/GenericRedoxEntityRenderer.java`
- `redox-mc-bridge/bridge-client/src/main/java/net/redoxmc/bridge/entity/GenericRedoxEntityRenderState.java`

**Estimated scope:** Extra Large | **Effort:** XL (5-8 días)

---

## Checkpoint: Entidades Funcionales
- [ ] Entidades vivas con atributos e IA operativas en servidor headless.
- [ ] Renderizado en cliente con Renderpearl sin advertencias de falta de modelo.

---

## Fase 10: Multi-Plataforma, Distribución y Capa Adaptadora Multi-Versión

### Task 10.1: Generalización de la Capa Adaptadora Multi-Versión
**Description:** Generalizar la abstracción de adaptadores basada en el diseño de `redox-mc-adapter-26-3`. El loader principal y la API pública interactúan exclusivamente a través de interfaces y traits agnósticos de versión.

**Acceptance criteria:**
- [ ] `redox-mc-loader` y `redox-mc-api` no contienen identificadores de clases ni firmas de 26.3 codificadas en duro fuera del adaptador.
- [ ] Soporte para versiones futuras requerirá únicamente implementar un nuevo crate adaptador sin reescribir el loader.

**Verification:**
- [ ] Build succeeds: `cargo check --all-targets`

**Dependencies:** Task 1.3, Task 9.2

**Files likely touched:**
- `redox-mc-adapter-26-3/src/lib.rs`
- `redox-mc-loader/src/adapter.rs`

**Estimated scope:** Medium | **Effort:** M (2-3 días)

---

### Task 10.2: Empaquetado y Distribución para el Usuario Final
**Description:** Diseñar la estrategia de instalación y ejecución para el jugador final en cliente y servidor:
- En cliente: compatibilidad con lanzadores (Prism, Modrinth, CurseForge y lanzador oficial mediante perfiles JSON que añadan `-agentpath` en JVM arguments).
- En servidor: script de arranque `run.sh`/`run.bat` que invoca `java -agentpath:libredox_mc_loader.so -jar server.jar nogui`.
- Creación automática de la carpeta `mods/` y documentación para instalación de binarios nativos.

**Acceptance criteria:**
- [ ] Se genera un archivo de perfil de lanzador compatible que configura los argumentos de la JVM automáticamente.
- [ ] El script de servidor inicia el servidor con el agente sin intervención manual del usuario.
- [ ] Si la carpeta `mods/` no existe, el agente la crea automáticamente al arrancar.

**Verification:**
- [ ] Manual check: Probar instalación en una instancia limpia siguiendo las instrucciones de la guía de distribución.

**Dependencies:** Task 10.1

**Files likely touched:**
- `distribution/launcher-profile.json`
- `distribution/server-run.sh`
- `distribution/README.md`
- `redox-mc-loader/src/fs.rs`

**Estimated scope:** Medium | **Effort:** M (2-3 días)

---

### Task 10.3: Matriz de Compilación Multi-Plataforma Final y Pipeline CI
**Description:** Extender la matriz de compilación configurada para Linux y Windows (Tarea 4.4) añadiendo soporte para macOS aarch64 (`.dylib`). Configurar un workflow de GitHub Actions que compile el workspace en las tres plataformas y ejecute pruebas unitarias y de integración en un servidor headless en CI.

**Acceptance criteria:**
- [ ] El loader, los puentes y el mod de ejemplo compilan limpiamente en Linux, Windows y macOS.
- [ ] Los tests automatizados en CI validan el arranque del servidor y la ejecución del mod de prueba.

**Verification:**
- [ ] Build succeeds: `cargo test --workspace` ejecutado en entorno Linux de CI.

**Dependencies:** Task 10.2

**Files likely touched:**
- `.github/workflows/ci.yml`
- `redox-mc-loader/build.rs`

**Estimated scope:** Medium | **Effort:** M (2-3 días)

---

## Checkpoint Final: Sistema Completo Listo para Producción
- [ ] Suite de pruebas pasando en todas las plataformas soportadas.
- [ ] Mod de ejemplo demostrando bloques, ítems, entidades, red, eventos y hot-reload.
- [ ] Paquetes de distribución listos para clientes y servidores.
