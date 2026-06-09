macro_rules! apply_to_all {
    ($( $item:expr ),*) => {
        for val in vec![ $( $item ),* ] {
                        println!("val = {}", val);
        }
    };
}
