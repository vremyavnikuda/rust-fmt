macro_rules! run_length_encode {
    ($($x:expr),*) => {{
        let mut result = Vec::new();
        let mut iter = [$($x),*].into_iter();
        let last_val = iter.next();
        if let Some(mut last) = last_val {
            let mut count = 1usize;
            for item in iter {
                if item == last {
                    count += 1;
                } else {
                    result.push((last, count));
                    last = item;
                    count = 1;
                }
            }
            result.push((last, count));
        }
        result
    }};
}

pub fn use_rle() {
        let encoded = run_length_encode!(1,1,1,2,2,3,4,4,4,4,5);
    println!("{:?}", encoded);
}
