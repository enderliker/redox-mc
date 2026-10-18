//! Example native Redox-MC mod written in 100% Rust.

#[no_mangle]
pub extern "C" fn redox_mod_init() -> i32 {
    println!("[test-mod] Native Rust mod initialized successfully via redox_mod_init()!");
    0
}

#[no_mangle]
pub extern "C" fn redox_mod_on_bootstrap() {
    println!("[test-mod] Hook received: Bootstrap.bootStrap() entered! Game is bootstrapping vanilla registries.");
}
