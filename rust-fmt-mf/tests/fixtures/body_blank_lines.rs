macro_rules! setup {
    ($name:ident, $value:expr) => {
        let $name = $value;

        println!("{}", $name);
    };
}

macro_rules! items {
    ($t:ident) => {
        struct $t;

        impl $t {
            fn new() -> Self {
                let x = 1;

                $t
            }
        }
    };
}

fn main() {
    setup!(a, 1);
}
