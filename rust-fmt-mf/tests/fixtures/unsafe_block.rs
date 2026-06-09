macro_rules! unsafe_access {
    ($base:expr, $( $offset:expr ),*) => {
        unsafe {
            let ptr = $base.as_ptr();
            $( let _ = *ptr.add( $offset ); )*
        }
    };
}
