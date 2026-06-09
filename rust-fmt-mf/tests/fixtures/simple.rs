macro_rules! bad_macro {
    ($x:expr) => {
        let val = $x + 1 * 2 / 3;
        println!("value: {}", val);
    };
}
