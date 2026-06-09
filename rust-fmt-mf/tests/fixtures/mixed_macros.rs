macro_rules! define_enum {
    ($name:ident { $($variant:ident $(($($field:ty),*))?),+ $(,)? }) => {
             pub enum $name {
            $($variant $(($($field),*))?,)+
        }
    };
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

define_enum! {
    MyGeneratedEnum  {
    Alpha,
       Beta(i32),
    } }

pub fn use_all(data: &DataFields) {
    let encoded = run_length_encode!(1,1,1,2);
    println!("{:?}", encoded);

    match MyGeneratedEnum::Alpha {
        MyGeneratedEnum::Alpha => println!("alpha"),
        _ => {}
    }

    println!("{}",
        data.name()
    );
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
