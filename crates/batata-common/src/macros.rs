/// Macro to generate `_or_default()` methods that return a field's value
/// or a default when the field is `None`, empty, or blank.
///
/// Eliminates boilerplate for the common pattern:
/// ```ignore
/// fn field_or_default(&self) -> &str {
///     self.field.as_deref().filter(|s| !s.is_empty()).unwrap_or(DEFAULT)
/// }
/// ```
///
/// Values are trimmed before the emptiness check, matching upstream
/// `NamespaceUtil.processNamespaceParameter`, which calls `trim()` on the
/// namespace before deciding whether it is blank.
///
/// # Usage
/// ```ignore
/// impl MyStruct {
///     impl_or_default!(namespace_id_or_default, namespace_id, "public");
///     impl_or_default!(pub, group_name_or_default, group_name, "DEFAULT_GROUP");
/// }
/// ```
#[macro_export]
macro_rules! impl_or_default {
    ($method_name:ident, $field:ident, $default:expr) => {
        fn $method_name(&self) -> &str {
            self.$field
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or($default)
        }
    };
    (pub, $method_name:ident, $field:ident, $default:expr) => {
        /// Returns the value of the associated field, falling back to its default when blank.
        pub fn $method_name(&self) -> &str {
            self.$field
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or($default)
        }
    };
}
