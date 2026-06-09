macro_rules! recursive_macro {
    ($val:expr,0) => {
        $val
    };
    ($val:expr,1) => {
        recursive_macro!($val * 2, 0)
    };
    ($val:expr,2) => {
        recursive_macro!($val * 2, 1)
    };
}

pub fn use_recursive() {
    let result = recursive_macro!(1, 2);
    println!("recursive result: {}", result);
}
