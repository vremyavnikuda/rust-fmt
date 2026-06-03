macro_rules! spawn_tasks {
    ($( $name:ident : $body:expr ),+ $(,)?) => {
        $( let handle = std::thread::spawn( move || { $body } ); )+
    };
}
