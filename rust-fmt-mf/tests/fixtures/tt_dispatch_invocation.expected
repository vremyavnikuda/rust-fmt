macro_rules! tt_based_dispatch {
    (bool $val:expr) => {
        if $val {
            "true"
        } else {
            "false"
        }
    };
    (int $val:expr) => {
        format!("{}", $val)
    };
    (str $val:expr) => {
        $val.to_string()
    };
}

pub fn use_tt_dispatch() {
    let s1 = tt_based_dispatch!(bool true);
    let s2 = tt_based_dispatch!(int 42);
    let s3 = tt_based_dispatch!(str "hello");
    println!("{} {} {}", s1, s2, s3);
}
