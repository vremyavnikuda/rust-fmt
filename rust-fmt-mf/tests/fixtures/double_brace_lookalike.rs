macro_rules! two_blocks {
    () => {{ let a = 1; }; { let b = 2; }};
}

macro_rules! real_double {
    ($x:expr) => {{ let y = $x; y }};
}
