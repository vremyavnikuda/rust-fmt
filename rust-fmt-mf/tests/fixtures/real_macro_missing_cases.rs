//! Macro forms missing from the original F5 workspace.

#[doc(hidden)]
#[macro_export]
macro_rules! exported_hidden_value {
    () => {
        42
    };
}
macro_rules! pass_item {
    ($item:item) => {
        $item
    };
}

pass_item! {
    #[derive(Debug)]
    pub      struct GeneratedByItem{pub           value:i32}
}
macro_rules!   collect_statements {
    ($($statement:stmt);* $(;)?) => {{
        $($statement)*
    }};
}

macro_rules!           borrowed_type {
    ($name:ident,$lt:lifetime,$ty:ty) => {
        pub   struct
        $name<$lt>{pub      value:&$lt $ty}
    };
}
borrowed_type!(BorrowedText,'a,str);
macro_rules! break_to_label {
    ($label:lifetime) => {
        break $label
    };
}
macro_rules! matches_param {
    ($value:expr,$pattern:pat_param) => {
        match $value {
            $pattern => true,
            _ => false,
        }
    };
}
macro_rules! matches_or_pattern {
    ($value:expr,$pattern:pat) => {
        matches!($value, $pattern)
    };
}
macro_rules! legacy_expression {
    ($value:expr_2021) => {{
        $value
    }};
}
macro_rules!   named_values {
    ($($name:ident=>$value:expr);+$(;)?) => {
        vec![$((stringify!($name), $value)),+]
    };
}

macro_rules! run_block {
    ($body:block) => {{
        (move || $body)()
    }};
}
macro_rules! generated_result_type {
    () => {
        Result<i32,String>
    };
}
pub type GeneratedResult = generated_result_type!();
macro_rules! generated_some_pattern {
    ($inner:pat_param) => {
        Some($inner)
    };
}
macro_rules! make_tripler {
    ($d:tt $name:ident) => {
        macro_rules!      $name{($d value:expr)=>{$d           value*3};}
    };
}

make_tripler!($      triple_generated);
macro_rules! raw_identifier_field {
    ($field:ident) => {
        pub struct RawIdentifier {
            pub $field: i32,
        }
    };
}
raw_identifier_field!(r#type);
pub fn crate_value() -> i32 {
    7
}
macro_rules! via_crate {
    () => {
        $crate::examples::macro_missing_cases::crate_value()
    };
}
macro_rules! commented_matcher {
    (

  $left:expr,   // left operand must survive collapsing
       $right:expr      $(,)?
) => {{
        /* block comment inside a macro body */
        $left + $right
    }};
}
pub fn exercise_missing_cases() {
    collect_statements! {
        let   mut
        total=0;total+=1;assert_eq!(total,1);
    }
    let borrowed = BorrowedText { value: "hello" };
    assert_eq!(borrowed.value, "hello");
    'outer: loop {
        break_to_label!('outer);
    }
    let _ = matches_param!(Some(1), Some(_));
    let _ = matches_or_pattern!(Some(1), Some(_) | None);
    let _ = legacy_expression!(1 + 2);
    let _ = named_values!(one=>1;two=>2;three=>3;);
    let _ = run_block!(
        {
            let value = 40;
            value + 2
        }
    );

    let typed: GeneratedResult = Ok(42);

    let _ = typed;
    let candidate = Some(5);
    match candidate {
        generated_some_pattern!(value) => {
            let _ = value;
        }
        None => {}
    }
    let _ = triple_generated!(14);
    let raw = RawIdentifier { r#type: 42 };

    let _ = raw.r#type;
    let _ = via_crate!();
    let _ = commented_matcher!(20, 22);
    let _ = exported_hidden_value!();
}
