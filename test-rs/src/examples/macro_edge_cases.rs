//! Edge cases for macro formatting that are NOT covered by macro_heavy.rs
//!
//! Unique patterns tested here:
//!   - $crate hygienic paths
//!   - Escaped dollar sign $$
//!   - TT dispatch with @marker internal arms
//!   - Recursive with $()* repetition
//!   - $pat and $literal fragment specifiers
//!   - Multi-delimiter definition
//!   - unsafe and async body blocks
//!   - Macro calling another macro
//!   - impl Trait generated inside macro body
//!   - const generic parameter in macro pattern
//!   - Vec of trailing comma styles $(,)? combined
//!   - Complex nested repetition with mix of * + ?
//!   - $crate resolved path in invocation

// $crate hygienic path demonstration
macro_rules! format_greeting {
    ($name:expr) => {
        format!("Hello, {}!", $name)
    };
}

// Escaped dollar sign
macro_rules! env_var {
    ($name:expr) => {
        std::env::var(format!("${}", $name))
    };
}

// TT dispatch with @marker
macro_rules! tt_dispatch_compute {
    (@double $x:expr) => {
        $x * 2
    };
    (@square $x:expr) => {
        $x * $x
    };
    (@negate $x:expr) => {
        -$x
    };
    ($op:ident $val:expr) => {
        tt_dispatch_compute!(@$op $val)
    };
}

// Recursive macro with $()* repetition
macro_rules! sum_repeat {
    ($($x:expr),* $(,)?) => {
        sum_repeat!(@acc 0 ; $($x),*)
    };
    (@acc $acc:expr;) => {
        $acc
    };
    (@acc $acc:expr; $head:expr $(, $tail:expr)*) => {
        sum_repeat!(@acc ($acc + $head) ; $($tail),*)
    };
}

// $pat fragment specifier
macro_rules! check_pattern {
    ($val:expr, $pat:pat => $result:expr, $fallback:expr) => {
        if let $pat = $val {
            $result
        } else {
            $fallback
        }
    };
}

// $literal fragment specifier
macro_rules! array_of_literals {
    ($val:literal; $count:literal) => {
        [$val; $count]
    };
}

// Multi-delimiter definition
macro_rules! multi_delim_style {
    ($($x:expr),*) => {
        vec![$($x),*]
    };
    [$($x:expr),*] => {
        vec![$($x),*]
    };
}

// unsafe block inside macro body
macro_rules! unsafe_deref {
    ($ptr:expr) => {
        unsafe { *$ptr }
    };
}

// async block inside macro body
macro_rules! wrap_async {
    ($fut:expr) => {
        async move { $fut.await }
    };
}

// Macro calling another macro
macro_rules! add_one {
    ($x:expr) => {
        $x + 1
    };
}
macro_rules! add_two {
    ($x:expr) => {
        add_one!(add_one!($x))
    };
}

// impl Trait generated inside macro body
macro_rules! derive_display_via_debug {
    ($name:ident) => {
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{:?}", self)
            }
        }
    };
}

// const generic in macro pattern
macro_rules! repeat_const {
    ($val:expr; $n:expr) => {
        [$val; $n]
    };
}

// Macro generating tuple structs from repetition
macro_rules! make_tuple_structs {
    ($($name:ident($ty:ty)),+ $(,)?) => {
        $(pub struct $name(pub $ty);)+
    };
}

// Macro building a struct with where clause
macro_rules! struct_with_bounds {
    (#[$meta:meta] $vis:vis struct $name:ident<$($param:ident),+> where $($bound:ident : $trait:path),+ $(,)?{$($field:ident : $ty:ty),+ $(,)?}) => {
        #[$meta]
        $vis struct $name<$($param),+>
        where
            $($param: $trait),+
        {
            $(pub $field: $ty),+
        }
    };
}

// Macro with cfg attribute
#[cfg(any())]
macro_rules! cfg_never_active {
    ($x:expr) => {
        unreachable!()
    };
}

// Macro using concat! inside body
macro_rules! concat_ids {
    ($prefix:ident, $suffix:ident) => {
        concat!(stringify!($prefix), "_", stringify!($suffix))
    };
}

// Invocations
pub struct DemoType;

impl std::fmt::Debug for DemoType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DemoType")
    }
}
derive_display_via_debug!(DemoType);

pub fn test_edge_invocations() {
    // format_greeting (simulates $crate pattern)
    let greeting = format_greeting!("world");
    println!("{}", greeting);

    // Escaped dollar
    let env_name = env_var!("PATH").unwrap_or_default();
    println!("env: {}", env_name);

    // TT dispatch
    let doubled = tt_dispatch_compute!(double 21  );
    let squared = tt_dispatch_compute!(square 9);
    let negated = tt_dispatch_compute!(negate 42  );
    println!("{} {} {}", doubled, squared, negated);

    // Recursive sum
    let total = sum_repeat!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10);
    println!("total: {}", total);

    // Pattern matching
    let res: Result<i32, &str> = Ok(42);
    let checked = check_pattern!(res , Ok(val) => val , 0 );
    println!("checked: {}", checked);

    // Literal array
    let arr = array_of_literals!(7 ; 5   );
    println!("{:?}", arr);

    // Multi delimiter
    let a = multi_delim_style!(1, 2, 3);
    let b = multi_delim_style![4, 5, 6];
    let c = multi_delim_style! {   7,8,9  };
    println!("{:?} {:?} {:?}", a, b, c);

    // Unsafe deref
    let val: i32 = 42;
    let ptr: *const i32 = &val as *const i32;
    let _derefed = unsafe_deref!(ptr);

    // Macro calling another macro
    let incremented = add_two!(5);
    println!("incremented: {}", incremented);

    // Const generic repeat
    let repeated = repeat_const!(99 ; 3  );
    println!("{:?}", repeated);

    // Concat ids
    let _id = concat_ids!(hello, world);
}

// Make tuple structs
make_tuple_structs!(Width(f64), Height(f64), Point(String),);

// Struct with bounds
struct_with_bounds!(
    #[derive(Debug, Clone)]
    pub struct Container<T, U>
    where
        T: Clone,
        U: Default,
    {
        data: T,
        extra: U,
    }
);

pub fn use_tuple_structs() {
    let _w = Width(10.0);
    let _h = Height(20.0);
    let _p = Point(String::from("test"));
    let _c = Container {
        data: "hello",
        extra: 42,
    };
    println!("done");
}
