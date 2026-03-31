//! Maintainer client trait contract tests (no server required).

/// Verify traits are object-safe (can be used as dyn)
#[test]
fn test_core_maintainer_service_object_safe() {
    fn _accept(_: &dyn batata_maintainer_client::CoreMaintainerService) {}
}

#[test]
fn test_config_maintainer_service_object_safe() {
    fn _accept(_: &dyn batata_maintainer_client::ConfigMaintainerService) {}
}

#[test]
fn test_naming_maintainer_service_object_safe() {
    fn _accept(_: &dyn batata_maintainer_client::NamingMaintainerService) {}
}

/// Verify trait exports are accessible from the crate root
#[test]
fn test_trait_exports() {
    // These compile only if the traits are properly exported
    use batata_maintainer_client::ConfigMaintainerService;
    use batata_maintainer_client::CoreMaintainerService;
    use batata_maintainer_client::NamingMaintainerService;

    // Just verify they exist as types
    let _: Option<Box<dyn CoreMaintainerService>> = None;
    let _: Option<Box<dyn ConfigMaintainerService>> = None;
    let _: Option<Box<dyn NamingMaintainerService>> = None;
}

/// Verify model types are accessible
#[test]
fn test_plugin_model_exports() {
    use batata_maintainer_client::model::plugin::{
        ConfigItemDefinition, PluginAvailability, PluginDetail, PluginInfo,
    };

    let _info = PluginInfo::default();
    let _detail = PluginDetail::default();
    let _def = ConfigItemDefinition::default();
    let _avail = PluginAvailability::default();
}
