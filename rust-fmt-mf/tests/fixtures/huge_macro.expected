macro_rules! huge_macro{
    ($(#[$attr:meta])*$vis:vis fn $name:ident($($arg:ident:$ty:ty),*$(,)?)->$ret:ty $body:block)=>{
        $(#[$attr])*$vis fn $name($($arg:$ty),*)->$ret $body
    };
}
