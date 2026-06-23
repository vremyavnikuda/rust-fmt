macro_rules! unsafe_deref {
    ($ptr:expr) => {
        unsafe { *$ptr }
    };
}
