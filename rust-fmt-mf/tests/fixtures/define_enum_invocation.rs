macro_rules! define_enum {
    ($name:ident { $($variant:ident $(($($field:ty),*))?),+ $(,)? }) => {
             pub enum $name {
            $($variant $(($($field),*))?,)+
        }
    };
}

define_enum! {
    MyGeneratedEnum  {
    Alpha,
       Beta(i32),
    Gamma(String, i32),
} }

pub fn use_generated_enum(val: MyGeneratedEnum) {
    match val {
           MyGeneratedEnum::Alpha => println!("alpha"),
        MyGeneratedEnum::Beta(n) => println!("beta: {}", n),
           MyGeneratedEnum::Gamma(s, n) => println!("gamma: {} {}", s, n),
    }
}
