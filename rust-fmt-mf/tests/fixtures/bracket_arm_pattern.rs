macro_rules! from_braces {
    { $x:expr } => {
        $x * 2
    };
    { $x:expr, $y:expr } => {
        $x + $y
    };
}

macro_rules! from_brackets {
    [ $x:expr ] => {
        $x
    };
}
