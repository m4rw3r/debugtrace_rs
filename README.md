# Debugtrace

This crate enables backtraces to be attached to values in `debug` and `test` builds without
incurring a cost in `release` or `bench` builds.

It uses the selected build-profile to determine if backtraces should be collected. On `debug`
and `test` backtraces are collected and will be printed if the value is printed using
`fmt::Debug` (`{:?}`). In `release` and `bench` profiles `Trace` will only be a newtype wrapper
making it zero-overhead.

Note that this crate operates under the same restrictions that `backtrace` does which means that
file and line-number information is not always available.

# Installation

```
[dependencies]
debugtrace = "0.1.0"
```

# Usage

```rust
extern crate debugtrace;

use debugtrace::Trace;

fn foo() -> Trace<u32> {
    Trace::new(123)
}

fn main () {
    let e = foo();

    println!("{:?}", e);
}
```

This will output something like the following when built with `debug` or `test` profiles:

```
123 at
   0       0x104f888d8 - foo::h83f9455bde0e24228Oe (src/main.rs:6)
   1       0x104f8978b - main::hd70e3ccce038deb2gPe (src/main.rs:10)
   2       0x104f9c312 - sys_common::unwind::try::try_fn::h4103587514840227558 (<unknown>:<unknown>)
   3       0x104f9ab48 - __rust_try (<unknown>:<unknown>)
   4       0x104f9c1b9 - rt::lang_start::h216753457f385fdaJCx (<unknown>:<unknown>)
   5       0x104f8fb69 - main (<unknown>:<unknown>)
```

When built using `release` or `bench` profiles it will not collect any backtrace and will only output:

```
123
```

# Features

* `backtrace`:

  This will force backtraces to be collected no matter what profile is used. It also includes
  the `Trace::resolve()` method to programmatically obtain the stacktrace.
