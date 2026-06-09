/// A documented macro with macro_export
#[macro_export]
macro_rules! create_struct {
    ($name:ident, $field:ident, $ty:ty) => {
        pub struct $name {
            pub $field: $ty,
        }
    };
}

/// Multiple arms, one empty
#[macro_export]
macro_rules! multi_doc {
    (empty) => {};
    (one $x:expr) => {
        $x
    };
}
