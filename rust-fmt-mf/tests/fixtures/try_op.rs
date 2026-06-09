macro_rules! try_chain {
    ($base:expr, $( $method:ident ),*) => {
        {
            let mut x = $base;
            $( x = x . $method () ? ; )*
            Ok(x)
        }
    };
}
