macro_rules! bad_macro {
    ($x:expr) => {
        println!("value: {}", $x)
    };
}

macro_rules! multi_arm_macro {
    (add $a:expr, $b:expr) => {
        $a + $b
    };
    (sub $a:expr, $b:expr) => {
        $a - $b
    };
    (mul $a:expr, $b:expr) => {
        $a * $b
    };
    (div $a:expr, $b:expr) => {
        $a / $b
    };
}

macro_rules! recursive_macro {
    ($val:expr,0) => {
        $val
    };
    ($val:expr,1) => {
        recursive_macro!($val * 2, 0)
    };
    ($val:expr,2) => {
        recursive_macro!($val * 2, 1)
    };
}

macro_rules! vec_of_strings {
    ($($x:expr),*) => {
        vec![$($x.to_string()),*]
    };
}

macro_rules! nested_macro_call {
    ($x:expr) => {
        bad_macro!(inline_macro_call!($x))
    };
    ($x:expr, $y:expr) => {
        bad_macro!($x);
        bad_macro!($y);
    };
}

macro_rules! inline_macro_call {
    ($x:expr) => {
        $x + 1
    };
}

pub fn use_bad_macros() {
    bad_macro!(42);
    bad_macro!("hello world");
    bad_macro!(1 + 2 + 3 + 4 + 5);
}

pub fn use_multi_arm() {
    let a = multi_arm_macro!(add 10, 20);
    let b = multi_arm_macro!(sub 100, 30);
    let c = multi_arm_macro!(mul 6, 7);
    let d = multi_arm_macro!(div 100, 4);
    println!("{} {} {} {}", a, b, c, d);
}

pub fn use_recursive() {
    let result = recursive_macro!(1, 2);
    println!("recursive result: {}", result);
}

pub fn use_vec_of_strings() {
    let v = vec_of_strings!("apple", "banana", "cherry", "date", "elderberry");
    println!("{:?}", v);
}

pub fn use_nested() {
    nested_macro_call!(41);
    nested_macro_call!(10, 20);
}

macro_rules! complex_pattern {
    (@inner $a:ident : $b:expr) => {
        let $a = $b;
    };
    ($name:ident{$($field:ident : $value:expr),+ $(,)?}) => {
        let $name = ($($value),+);
    };
}

pub fn use_complex_pattern() {
    complex_pattern!( @inner x: 42 );
    println!("x = {}", x);
    let _result = (10 + 1 + 2 + 3) * 2 * 3;
}

macro_rules! huge_macro {
    ($(#[$attr:meta])* $vis:vis fn $name:ident($($arg:ident : $ty:ty),* $(,)?) -> $ret:ty $body:block) => {
        $(#[$attr])* $vis fn $name($($arg: $ty),*) -> $ret $body
    };
}

macro_rules! multi_line_invocation {
    () => {
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9]
    };
}

pub fn use_multi_line() {
    let v = multi_line_invocation!();
    println!("{:?}", v);
}

macro_rules! token_tree_macro {
    ($($tt:tt)*) => {
        vec![$(stringify!($tt)),*]
    };
}

pub fn use_token_tree() {
    let v = token_tree_macro!(a + b * c / d);
    println!("{:?}", v);
}

macro_rules! count_exprs {
    () => {
        0usize
    };
    ($head:expr $(, $tail:expr)*) => {
        1usize + count_exprs!($($tail),*)
    };
}

pub fn use_count() {
    let count = count_exprs!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10);
    println!("count: {}", count);
}

macro_rules! field_accessor {
    ($struct_name:ident, $($field:ident : $ty:ty),+) => {
        impl $struct_name {
            $(
                pub fn $field(&self) -> &$ty {
                    &self.$field
                }
            )+
        }
    };
}

pub struct DataFields {
    pub name: String,
    pub age: u32,
    pub email: String,
    pub active: bool,
}

field_accessor!( DataFields, name: String, age: u32, email: String, active: bool );

pub fn use_accessors(data: &DataFields) {
    println!(
        "{} {} {} {}",
        data.name(),
        data.age(),
        data.email(),
        data.active()
    );
}

macro_rules! format_madness {
    ($fmt:expr, $($arg:expr),+ $(,)?) => {
        format!($fmt, $($arg),+)
    };
}

pub fn use_format_madness() {
    let s = format_madness!("{} + {} = {}", 1, 2, 3);
    println!("{}", s);
}

macro_rules! repeat_pattern {
    ($x:expr; $n:expr) => {
        std::iter::repeat($x).take($n).collect::<Vec<_>>()
    };
}

pub fn use_repeat() {
    let v: Vec<i32> = repeat_pattern!(42; 10);
    println!("{:?}", v);
}

macro_rules! tt_recurse {
    ($head:tt $($tail:tt)*) => {
        stringify!($head)
    };
    ($head:tt $mid:tt $tail:tt) => {
        (stringify!($head), stringify!($mid), stringify!($tail))
    };
}

pub fn use_tt_recurse() {
    let _a = tt_recurse!( hello world );
    let _b = tt_recurse!( a b c d e f g );
}

macro_rules! define_enum {
    ($name:ident{$($variant:ident $(($($field:ty),*))?),+ $(,)?}) => {
        pub enum $name {
            $($variant $(($($field),*))?,)+
        }
    };
}

define_enum!(
    MyGeneratedEnum  {
        Alpha,
        Beta(i32),
        Gamma(String, i32),
    }
);

pub fn use_generated_enum(val: MyGeneratedEnum) {
    match val {
        MyGeneratedEnum::Alpha => println!("alpha"),
        MyGeneratedEnum::Beta(n) => println!("beta: {}", n),
        MyGeneratedEnum::Gamma(s, n) => println!("gamma: {} {}", s, n),
    }
}

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
    let encoded = run_length_encode!(1, 1, 1, 2, 2, 3, 4, 4, 4, 4, 5);
    println!("{:?}", encoded);
}

pub fn builtin_macro_mess() {
    let v = vec![
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
    ];
    println!("{:?}", v);
    println!("formatted: {}", "hello");
    println!(
        "multi
        line
        string"
    );
}

pub fn assert_macro_mess() {
    let x = 42;
    let y = 42;
    assert_eq!(x, y);
    assert_ne!(x, 0);
    assert!(x > 0 && x < 100);
}

pub fn format_args_mess() {
    let s = format!(
        "hello {name} you are {age} years old",
        name = "world",
        age = 42
    );
    println!("{}", s);
}

pub fn concat_mess() {
    let s = concat!("hello", ",", " ", "world", "!");
    println!("{}", s);
}

pub fn include_str_mock() {
    let _s = "included content";
    println!("{}", _s);
}

pub fn macro_repetition_in_fn() {
    let v: Vec<i32> = vec![0; 100];
    let sum: i32 = v.iter().map(|x| x * 2).filter(|x| x % 3 == 0).sum();
    println!("sum = {}", sum);
}

macro_rules! cascading_macro {
    ($x:expr) => {
        cascading_macro!($x, $x)
    };
    ($x:expr, $y:expr) => {
        cascading_macro!($x, $y, $x + $y)
    };
    ($x:expr, $y:expr, $z:expr) => {
        $x + $y + $z
    };
}

pub fn use_cascading() {
    let r = cascading_macro!(5);
    println!("cascading: {}", r);
}

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

macro_rules! long_macro_invocation {
    ($($x:expr),+ $(,)?) => {
        ($($x),+)
    };
}

pub fn very_long_macro_call() {
    let _t = long_macro_invocation!(
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
        26, 27, 28, 29, 30,
    );
}

macro_rules! stringify_many {
    ($($x:tt)*) => {
        ($(stringify!($x),)*)
    };
}

pub fn use_stringify_many() {
    let _s = stringify_many!(
        fn hello(x: i32) -> i32 {
            x + 1
        }
    );
}
