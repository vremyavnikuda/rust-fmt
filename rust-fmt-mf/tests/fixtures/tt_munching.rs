macro_rules! token_tree_macro {
    ($($tt:tt)* ) => {
            vec![ $( stringify!($tt) ),* ]
    };
}

macro_rules! tt_recurse {
    ($head:tt $($tail:tt)*) => {
        stringify!($head)
    };
    ($head:tt $mid:tt $tail:tt) => {
        (stringify!($head), stringify!($mid), stringify!($tail))
    };
}

macro_rules! count_exprs {
    () => { 0usize };
    ( $head:expr $(, $tail:expr )* ) => { 1usize + count_exprs!( $( $tail ),* ) };
}
