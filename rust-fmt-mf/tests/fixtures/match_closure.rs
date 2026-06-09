macro_rules! process {
    ($val:expr, $( $f:ident ),*) => {
        match $val {
            Some(x) => $( $f(x) ),*,
            None => { let noop = || {}; noop() }
        }
    };
}
