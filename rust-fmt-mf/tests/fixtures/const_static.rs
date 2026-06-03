macro_rules! declare_consts {
    ($( $name:ident : $ty:ty = $val:expr ),+ $(,)?) => {
        $( const $name : $ty = $val ; )+
    };
}
