macro_rules! impl_trait {
    ($ty:ident, $trait:path, $( $method:ident : $ret:ty ),*) => {
        impl $trait for $ty {
                    $( fn $method(&self) -> $ret { unimplemented!() } )*
        }
    };
}
