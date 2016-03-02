use std::env;

fn main() {
    let enable_trace = match env::var("PROFILE").unwrap().as_ref() {
        "debug" => true,
        "test"  => true,
        _       => false,
    };

    if enable_trace {
        // We enable the stacktrace debug-logging here:
        println!("cargo:rustc-cfg=feature=\"logtrace\"");
    }
}
