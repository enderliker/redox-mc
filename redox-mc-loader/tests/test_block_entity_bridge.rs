#[test]
fn test_block_entity_class_bytes_validity() {
    use redox_mc_loader::class_injector::{
        GENERIC_REDOX_BLOCK_ENTITY_BYTES, GENERIC_REDOX_ENTITY_BLOCK_BYTES,
    };
    assert!(!GENERIC_REDOX_BLOCK_ENTITY_BYTES.is_empty());
    assert!(!GENERIC_REDOX_ENTITY_BLOCK_BYTES.is_empty());

    // Magic bytes for Java classfile 0xCAFEBABE
    assert_eq!(
        &GENERIC_REDOX_BLOCK_ENTITY_BYTES[0..4],
        &[0xCA, 0xFE, 0xBA, 0xBE]
    );
    assert_eq!(
        &GENERIC_REDOX_ENTITY_BLOCK_BYTES[0..4],
        &[0xCA, 0xFE, 0xBA, 0xBE]
    );
}
