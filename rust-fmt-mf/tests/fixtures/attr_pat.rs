macro_rules! with_attrs {
    (
        $( #[$attr:meta] )*
        $vis:vis fn $name:ident ( $($arg:ident : $ty:ty),* ) -> $ret:ty
    ) => {
        $( #[$attr] )*
        $vis fn $name ( $( $arg : $ty ),* ) -> $ret
    };
}
