macro_rules! check_pattern {
    ($val:expr, $pat:pat => $result:expr, $fallback:expr) => {
        if let $pat = $val {
            $result
        } else {
            $fallback
        }
    };
}
