macro_rules! make_fn {
    ( $vis:vis fn $name:ident ( $( $arg:ident : $ty:ty ),* ) -> $ret:ty $body:block ) => {
            $vis fn $name ( $( $arg : $ty ),* ) -> $ret $body
    };
}
