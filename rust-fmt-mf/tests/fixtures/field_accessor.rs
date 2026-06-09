macro_rules! field_accessor {
    ( $struct_name:ident, $( $field:ident : $ty:ty ),+ ) => {
            impl $struct_name {
            $(
                pub fn $field( &self) -> &$ty {
                        &self.$field
                }
            )+
        }
    };
}
