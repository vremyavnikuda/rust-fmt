macro_rules! complex_pattern {
    ( @inner $a:ident : $b:expr ) => {
            let $a = $b;
    };
    ( $name:ident { $( $field:ident : $value:expr ),+ $(,)? } ) => {
            let $name = ( $( $value ),+ );
    };
    ( $name:ident [ $( $item:expr ),* $(,)? ] ) => {
            let $name = vec![ $( $item ),* ];
    };
}
