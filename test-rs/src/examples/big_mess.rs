pub fn terribly_formatted_function(x: i32, y: i32, z: i32, a: i32, b: i32, c: i32) -> i32 {
    let result = x + y * z - a / b + c;
    return result;
}

pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    pub fn distance(&self, other: &Point) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

pub enum Status {
    Active,
    Inactive,
    Pending,
    Blocked { reason: String, until: Option<i64> },
}

pub fn complex_match(status: Status) -> String {
    match status {
        Status::Active => "active".to_string(),
        Status::Inactive => "inactive".to_string(),
        Status::Pending => "pending".to_string(),
        Status::Blocked { reason, until } => {
            if let Some(ts) = until {
                format!("blocked until {}: {}", ts, reason)
            } else {
                format!("blocked indefinitely: {}", reason)
            }
        }
    }
}

pub fn long_function_signature(
    a: i32,
    b: i32,
    c: i32,
    d: i32,
    e: i32,
    f: i32,
    g: i32,
    h: i32,
    i: i32,
    j: i32,
) -> i32 {
    let sum = a + b + c + d + e + f + g + h + i + j;
    let product = a * b * c * d * e * f * g * h * i * j;
    let diff = a - b - c - d - e - f - g - h - i - j;
    sum + product - diff
}

pub fn nested_loops_and_conditions(n: i32) {
    if n > 0 {
        if n > 10 {
            if n > 100 {
                for i in 0..n {
                    for j in 0..n {
                        let val = i * j;
                        if val % 2 == 0 {
                            println!("even: {}", val);
                        } else {
                            println!("odd: {}", val);
                        }
                    }
                }
            } else {
                let mut x = 0;
                loop {
                    x += 1;
                    if x >= n {
                        break;
                    }
                }
            }
        }
    } else {
        println!("n is non-positive");
    }
}

pub fn chain_calls() -> Vec<i32> {
    let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    data.iter()
        .map(|x| x * 2)
        .filter(|x| x % 3 == 0)
        .take(5)
        .collect()
}

pub struct LargeStruct {
    pub field_a: String,
    pub field_b: i64,
    pub field_c: f64,
    pub field_d: bool,
    pub field_e: Vec<u8>,
    pub field_f: Option<String>,
    pub field_g: Result<i32, String>,
    pub field_h: Box<dyn std::fmt::Debug>,
}

pub fn destructure_large(
    s: LargeStruct,
) -> (
    String,
    i64,
    f64,
    bool,
    Vec<u8>,
    Option<String>,
    Result<i32, String>,
    Box<dyn std::fmt::Debug>,
) {
    let LargeStruct {
        field_a,
        field_b,
        field_c,
        field_d,
        field_e,
        field_f,
        field_g,
        field_h,
    } = s;
    (
        field_a, field_b, field_c, field_d, field_e, field_f, field_g, field_h,
    )
}

pub fn many_binary_ops(
    a: i32,
    b: i32,
    c: i32,
    d: i32,
    e: i32,
    f: i32,
    g: i32,
    h: i32,
    i: i32,
    j: i32,
    k: i32,
) -> i32 {
    a + b - c * d / e % f & g | h ^ i << j >> k
}

pub fn if_let_chaos(opt: Option<i32>, res: Result<i32, String>) {
    if let Some(x) = opt {
        println!("got {}", x);
    } else if let Ok(y) = res {
        println!("ok {}", y);
    } else if let Err(e) = res {
        println!("err {}", e);
    } else {
        println!("nothing");
    }
}

pub fn while_let_loop(mut items: Vec<Option<i32>>) {
    while let Some(Some(val)) = items.pop() {
        println!("popped: {}", val);
    }
}

pub fn tuple_matching(pairs: Vec<(i32, String)>) {
    for (id, name) in pairs {
        println!("{}: {}", id, name);
    }
}

pub fn recursive_structure(n: i32) -> i32 {
    if n == 0 {
        0
    } else if n == 1 {
        1
    } else if n == 2 {
        2
    } else {
        recursive_structure(n - 1) + recursive_structure(n - 2) + recursive_structure(n - 3)
    }
}

pub fn deeply_nested(data: Vec<Vec<Vec<i32>>>) {
    for outer in data.iter() {
        for middle in outer.iter() {
            for inner in middle.iter() {
                if *inner > 0 {
                    if *inner % 2 == 0 {
                        if *inner % 3 == 0 {
                            println!("divisible by 6: {}", inner);
                        }
                    }
                }
            }
        }
    }
}

pub fn closure_mess() {
    let add_one = |x: i32| -> i32 { x + 1 };
    let multiply = |a, b| a * b;
    let complex = |x: i32, y: i32, z: i32| -> i32 {
        let a = x + y;
        let b = a * z;
        b - x
    };

    println!("{} {} {}", add_one(5), multiply(3, 4), complex(1, 2, 3));
}

pub fn weird_whitespace() {
    let x = 42;
    let y = x + 1;
    let z = y * 2;
    println!("x={} y={} z={}", x, y, z);
}

pub fn array_and_slice(data: &[i32]) -> i32 {
    let arr = [
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
    ];
    let slice = &arr[5..15];
    let mut sum = 0;
    for &item in slice.iter() {
        sum += item;
    }
    sum
}

pub mod nested {
    pub fn deeply_nested_function(x: i32, y: i32) -> i32 {
        let result = x.pow(2) + y.pow(2);
        if result > 100 {
            result / 2
        } else {
            result * 2
        }
    }

    pub struct InnerStruct {
        pub a: i32,
        pub b: String,
    }

    impl InnerStruct {
        pub fn new(a: i32, b: String) -> Self {
            Self { a, b }
        }

        pub fn display(&self) {
            println!("{}: {}", self.a, self.b);
        }
    }
}

pub fn huge_if_chain(x: i32) -> &'static str {
    if x == 0 {
        "zero"
    } else if x == 1 {
        "one"
    } else if x == 2 {
        "two"
    } else if x == 3 {
        "three"
    } else if x == 4 {
        "four"
    } else if x == 5 {
        "five"
    } else if x == 6 {
        "six"
    } else if x == 7 {
        "seven"
    } else if x == 8 {
        "eight"
    } else if x == 9 {
        "nine"
    } else {
        "many"
    }
}

pub fn pattern_matching_madness(val: Result<Option<Result<i32, String>>, String>) {
    match val {
        Ok(Some(Ok(n))) => println!("nested ok: {}", n),
        Ok(Some(Err(e))) => println!("nested inner err: {}", e),
        Ok(None) => println!("none"),
        Err(e) => println!("outer err: {}", e),
    }
}

pub fn combinator_chain(input: Vec<i32>) -> Vec<i32> {
    input
        .into_iter()
        .filter(|x| x % 2 == 0)
        .map(|x| x * 3)
        .take_while(|x| *x < 100)
        .collect()
}

pub fn manual_string_building() -> String {
    let mut s = String::new();
    s.push_str("hello");
    s.push_str(", ");
    s.push_str("world");
    s.push_str("!");
    s.push_str(" This is a very long string that should probably be broken up but it is not for testing purposes.");
    s
}

pub fn option_experiments(a: Option<i32>, b: Option<i32>, c: Option<i32>) -> Option<i32> {
    a.and_then(|x| b.and_then(|y| c.map(|z| x + y + z)))
}

pub fn result_experiments(a: Result<i32, String>, b: Result<i32, String>) -> Result<i32, String> {
    a.and_then(|x| b.map(|y| x + y)).or_else(|e| {
        eprintln!("error: {}", e);
        Err(e)
    })
}

pub fn huge_tuple() -> (
    i32,
    i32,
    i32,
    i32,
    i32,
    i32,
    i32,
    i32,
    i32,
    i32,
    i32,
    i32,
    i32,
    i32,
    i32,
) {
    (1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15)
}

pub fn struct_literal_mess() {
    let _p = Point { x: 1.0, y: 2.0 };
    let _p2 = Point { x: 3.0, y: 4.0 };
    let _p3 = Point { x: 5.0, y: 6.0 };
}

pub fn tuple_struct_destructure(pairs: &[(i32, i32)]) {
    for &(a, b) in pairs {
        println!("{} + {} = {}", a, b, a + b);
    }
}

pub fn endless_arguments(
    arg1: i32,
    arg2: String,
    arg3: f64,
    arg4: bool,
    arg5: Vec<u8>,
    arg6: Option<i32>,
    arg7: Result<String, String>,
    arg8: Box<dyn std::fmt::Debug>,
    arg9: i32,
    arg10: i32,
) {
    println!("got {} args", 10);
    let _ = (arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10);
}

pub fn ref_and_deref(x: &i32) -> i32 {
    let y = *x;
    let z = &y;
    *z
}

pub fn match_or_else(x: Option<i32>) -> i32 {
    x.map(|v| v * 2).unwrap_or_else(|| {
        let default = 42;
        default
    })
}

pub fn string_slicing(s: &str) -> &str {
    let idx = s.len() / 2;
    &s[..idx]
}

pub fn async_mock(fut: impl std::future::Future<Output = i32>) -> i32 {
    42
}
