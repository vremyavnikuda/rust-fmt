macro_rules! outer {
    () => {
        macro_rules! inner {
            () => {
                42
            };
        }
    };
    ($x:expr) => {
        macro_rules! inner_with_arg {
            ($y:expr) => {
                $x + $y
            };
        }
    };
}
