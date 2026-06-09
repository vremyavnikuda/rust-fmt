macro_rules! nested_matrix {
    ( $( $( $x:expr );* ),+ ) => {
        vec![ $( vec![ $( $x ),* ] ),+ ]
    };
}
