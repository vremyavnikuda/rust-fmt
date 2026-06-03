mod examples;

macro_rules! with_optional {
    ($val:expr $(, $extra:expr)? $(,)?) => {
        let result = $val;
        $(let _ = $extra;)?
        result
    };
}

macro_rules! brace_pattern {
    {$x:expr} => {
        $x * 2
    };
    {$x:expr, $y:expr} => {
        $x + $y
    };
}

macro_rules! bracket_pattern {
    [$x:expr] => {
        $x
    };
}

fn main() {
    let x = 1 + 2 * 3;
    println!("Hello, world!");

    if x > 5 {
        println!("x is big");
    } else {
        println!("x is small");
    }

    let arr = [1, 2, 3, 4];
    for item in arr.iter() {
        println!("{}", item);
    }

    let result = add(x, 10);
    println!("Result: {}", result);

    examples::big_mess::terribly_formatted_function(1, 2, 3, 4, 5, 6);
    examples::complex_types::generic_function(42, "hello");
    examples::macro_heavy::use_bad_macros();

    // Test $(?) in invocation
    let _x = with_optional!(42);
    let _y = with_optional!(42, extra);

    // Test {pattern} and [pattern] invocations
    let _a = brace_pattern! { 21 };
    let _b = brace_pattern! { 10, 20 };
    let _c = bracket_pattern![7];
}

fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}
