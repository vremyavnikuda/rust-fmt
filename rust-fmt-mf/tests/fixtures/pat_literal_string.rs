macro_rules! match_literals {
    ("exit") => {
        std::process::exit(0)
    };
    ($p:pat @ $x:expr) => {
        if let $p = $x { true } else { false }
    };
    ($l:literal) => {
        println!("lit: {}", $l)
    };
}
