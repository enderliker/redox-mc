use redox_mc_api::resource::{
    clear_registered_resources, get_registered_resources, redox_mod_get_resource_at,
    redox_mod_get_resource_count, register_asset_resource, register_data_resource,
    register_resource,
};

#[test]
fn test_register_and_query_virtual_resources() {
    clear_registered_resources();
    assert_eq!(redox_mod_get_resource_count(), 0);

    register_data_resource("testmod", "recipes/ruby.json", b"{\"type\":\"crafting\"}");
    register_asset_resource(
        "testmod",
        "models/item/ruby.json",
        b"{\"parent\":\"item/generated\"}",
    );
    register_resource(
        "data",
        "custommod",
        "tags/items/gems.json",
        b"{\"values\":[]}",
    );

    assert_eq!(redox_mod_get_resource_count(), 3);

    let resources = get_registered_resources();
    assert_eq!(resources.len(), 3);
    assert_eq!(resources[0].pack_type, "data");
    assert_eq!(resources[0].namespace, "testmod");
    assert_eq!(resources[0].path, "recipes/ruby.json");
    assert_eq!(resources[0].content, b"{\"type\":\"crafting\"}");

    assert_eq!(resources[1].pack_type, "assets");
    assert_eq!(resources[1].namespace, "testmod");

    // Test C-ABI exports
    unsafe {
        let mut pack_type = redox_mc_abi::types::FfiStr::EMPTY;
        let mut namespace = redox_mc_abi::types::FfiStr::EMPTY;
        let mut path = redox_mc_abi::types::FfiStr::EMPTY;
        let mut data_ptr: *const u8 = std::ptr::null();
        let mut data_len: usize = 0;

        let status = redox_mod_get_resource_at(
            0,
            &mut pack_type,
            &mut namespace,
            &mut path,
            &mut data_ptr,
            &mut data_len,
        );
        assert_eq!(status, redox_mc_abi::types::FfiStatus::Success);
        assert_eq!(pack_type.as_str(), Ok("data"));
        assert_eq!(namespace.as_str(), Ok("testmod"));
        assert_eq!(path.as_str(), Ok("recipes/ruby.json"));
        assert_eq!(data_len, b"{\"type\":\"crafting\"}".len());
        assert!(!data_ptr.is_null());
        let slice = std::slice::from_raw_parts(data_ptr, data_len);
        assert_eq!(slice, b"{\"type\":\"crafting\"}");

        // Index out of bounds
        let status_oob = redox_mod_get_resource_at(
            99,
            &mut pack_type,
            &mut namespace,
            &mut path,
            &mut data_ptr,
            &mut data_len,
        );
        assert_eq!(status_oob, redox_mc_abi::types::FfiStatus::NotFound);
    }

    clear_registered_resources();
    assert_eq!(redox_mod_get_resource_count(), 0);
}
