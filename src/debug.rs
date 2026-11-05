//! Terminal debugging macros: dump!(), dd!(), ddd!(), dddd!()
//! Provides instant, formatted console inspection with line numbers and die semantics.

#[macro_export]
macro_rules! dump {
    ($($val:expr),+ $(,)?) => {
        $(
            eprintln!("\x1b[36m┌── [dump] \x1b[1m{}\x1b[0m \x1b[90m({}:{})\x1b[0m", stringify!($val), file!(), line!());
            eprintln!("\x1b[36m│\x1b[0m {:#?}", &$val);
            eprintln!("\x1b[36m└──\x1b[0m");
        )+
    };
}

#[macro_export]
macro_rules! dd {
    ($($val:expr),+ $(,)?) => {
        $crate::dump!($($val),+);
        eprintln!("\x1b[1;31m[dd] Dump & Die called at {}:{}\x1b[0m", file!(), line!());
        if cfg!(test) {
            panic!("dd!() called at {}:{}", file!(), line!());
        } else {
            std::process::exit(1);
        }
    };
}

#[macro_export]
macro_rules! ddd {
    ($($val:expr),+ $(,)?) => {
        $crate::dump!($($val),+);
        eprintln!("\x1b[1;35m[ddd] Deep Dump & Die (Ignition Debug) called at {}:{}\x1b[0m", file!(), line!());
        if cfg!(test) {
            panic!("ddd!() called at {}:{}", file!(), line!());
        } else {
            std::process::exit(1);
        }
    };
}

#[macro_export]
macro_rules! dddd {
    ($($val:expr),+ $(,)?) => {
        $crate::ddd!($($val),+);
    };
}
