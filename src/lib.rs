//! This crate enables backtraces to be attached to values in `debug` and `test` builds without
//! incurring a cost in `release` or `bench` builds.
//!
//! It uses the selected build-profile to determine if backtraces should be collected. On `debug`
//! and `test` backtraces are collected and will be printed if the value is printed using
//! `fmt::Debug` (`{:?}`). In `release` and `bench` profiles `Trace` will only be a newtype wrapper
//! making it zero-overhead.
//!
//! Note that this crate operates under the same restrictions that `backtrace` does which means that
//! file and line-number information is not always available.
//!
//! # Installation
//!
//! ```toml
//! [dependencies]
//! debugtrace = "0.1.0"
//! ```
//!
//! # Usage
//!
//! ```rust
//! extern crate debugtrace;
//!
//! use debugtrace::Trace;
//!
//! fn foo() -> Trace<u32> {
//!     Trace::new(123)
//! }
//!
//! fn main () {
//!     let e = foo();
//!
//!     println!("{:?}", e);
//! }
//! ```
//!
//! This will output something like the following when built with `debug` or `test` profiles:
//!
//! ```text
//! 123 at
//!    0       0x104f888d8 - foo::h83f9455bde0e24228Oe (src/main.rs:6)
//!    1       0x104f8978b - main::hd70e3ccce038deb2gPe (src/main.rs:10)
//!    2       0x104f9c312 - sys_common::unwind::try::try_fn::h4103587514840227558 (<unknown>:<unknown>)
//!    3       0x104f9ab48 - __rust_try (<unknown>:<unknown>)
//!    4       0x104f9c1b9 - rt::lang_start::h216753457f385fdaJCx (<unknown>:<unknown>)
//!    5       0x104f8fb69 - main (<unknown>:<unknown>)
//! ```
//!
//! When built using `release` or `bench` profiles it will not collect any backtrace and will only output:
//!
//! ```text
//! 123
//! ```
//!
//! # Features
//!
//! * `backtrace`:
//!
//!   This will force backtraces to be collected no matter what profile is used. It also includes
//!   the `Trace::resolve()` method to programmatically obtain the stacktrace.

// If either the feature debug_print_trace or backtrace is set, include features for tracing:
#[cfg(any(feature="debug_print_trace", feature="backtrace"))]
extern crate backtrace;

/// Type-alias to make it easier to slot in tracing of errors.
pub type Result<T, E> = std::result::Result<T, Trace<E>>;

pub use trace::Trace;

// Only enable obtaining the trace if backtrace is manually set:
#[cfg(feature="backtrace")]
pub use trace::StackFrame;

#[cfg(any(feature="debug_print_trace", feature="backtrace"))]
mod trace {
    use std::fmt;
    use std::os::raw;
    use std::ops::{Deref, DerefMut};
    use std::cmp::Ordering;
    use std::hash::{Hash, Hasher};
    use std::borrow::Cow;

    use backtrace;

    /// Wrapper type containing a value and the backtrace to the address where it was wrapped.
    ///
    /// It is transparent and forwards `Hash`, `PartialOrd`, `Ord`, `PartialEq` and `Eq` to the
    /// wrapped type. `Deref` and `DerefMut` implementations are provided to make it somewhat easy
    /// to work with the contained type.
    #[derive(Clone)]
    pub struct Trace<T>(T, Vec<*mut raw::c_void>);

    /// Enables any error to automatically be wrapped in `Trace<E>` when `Result<T, E>` is used in
    /// the `try!` macro.
    impl<T> From<T> for Trace<T> {
        #[inline(always)]
        fn from(t: T) -> Self {
            Trace::new(t)
        }
    }

    /// Implementation of debug which debug-prints the inner type followed by the stacktrace to the
    /// address where the `Trace` was created.
    impl<T: fmt::Debug> fmt::Debug for Trace<T> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            try!(write!(f, "{:?} at\n", &self.0));

            for (i, frame) in resolve(self).into_iter().enumerate() {
                // TODO: is there any nicer way to get a decent indent?
                try!(write!(f, "{:>4}  {:?}\n", i, &frame));
            }

            Ok(())
        }
    }

    impl<T> Deref for Trace<T> {
        type Target = T;

        fn deref(&self) -> &T {
            &self.0
        }
    }

    impl<T> DerefMut for Trace<T> {
        fn deref_mut(&mut self) -> &mut T {
            &mut self.0
        }
    }

    impl<T: Hash> Hash for Trace<T> {
        fn hash<H>(&self, state: &mut H)
          where H: Hasher {
            Hash::hash(&self.0, state)
        }
    }

    impl<T: PartialOrd> PartialOrd for Trace<T> {
        fn partial_cmp(&self, rhs: &Trace<T>) -> Option<Ordering> {
            PartialOrd::partial_cmp(&self.0, &rhs.0)
        }
    }

    impl<T: Ord> Ord for Trace<T> {
        fn cmp(&self, rhs: &Self) -> Ordering {
            Ord::cmp(&self.0, &rhs.0)
        }
    }

    impl<T: PartialEq> PartialEq<Trace<T>> for Trace<T> {
        fn eq(&self, rhs: &Trace<T>) -> bool {
            self.0 == rhs.0
        }
    }

    impl<T: Eq> Eq for Trace<T> {}

    impl<T> Trace<T> {
        /// Creates a new stack-trace wrapping a value.
        ///
        /// ```
        /// use debugtrace::Trace;
        ///
        /// fn foo() -> Trace<()> {
        ///     Trace::new(())
        /// }
        ///
        /// let t = foo();
        ///
        /// // Prints "() at:" followed by a backtrace in debug and test profiles
        /// // Will also print the backtrace if the `backtrace` feature is on
        /// // independent of profile
        /// println!("{:?}", t);
        /// ```
        // Inline never is required to accurately obtain a trace
        #[inline(never)]
        pub fn new(t: T) -> Self {
            let mut v = Vec::new();
            let mut n = 0;

            backtrace::trace(&mut |frame| {
                // `backtrace::trace` is never inlined and same goes for `Trace::new`, skip those
                // two:
                if n > 1 {
                    v.push(frame.ip());
                }

                n = n + 1;

                true
            });

            Trace(t, v)
        }

        /// Discards the information about where this trace was constructed and yields the wrapped
        /// data.
        ///
        /// ```
        /// use debugtrace::Trace;
        ///
        /// let t = Trace::new("my error");
        ///
        /// assert_eq!(t.unwrap(), "my error");
        /// ```
        #[inline(always)]
        pub fn unwrap(self) -> T { self.0 }

        /// Resolves all of the symbols in the stack-trace and returns a list of them with the
        /// first being the location of the invocation of `Trace::new`.
        ///
        /// This method is only available if the feature `backtrace` is manually specified.
        // Only enable if manually activated:
        #[cfg(feature="backtrace")]
        pub fn resolve(&self) -> Vec<StackFrame> {
            resolve(self)
        }
    }

    /// Resolves the symbols of the stack-trace from the list of instruction-pointers.
    fn resolve<T>(t: &Trace<T>) -> Vec<StackFrame> {
        t.1.iter().map(|&ip| {
            let mut f = StackFrame { ip: ip, name: None, addr: None, file: None, line: None };

            backtrace::resolve(ip, &mut |sym| {
                f.name = sym.name().map(String::from_utf8_lossy).map(|mangled| {
                    let mut name = String::new();

                    match backtrace::demangle(&mut name, &mangled) {
                        Ok(()) => name,
                        Err(_) => mangled.into_owned(),
                    }
                });
                f.addr = sym.addr();
                f.file = sym.filename().map(String::from_utf8_lossy).map(Cow::into_owned);
                f.line = sym.lineno();
            });

            f
        }).collect()
    }

    /// A stack frame with information gathered from the runtime.
    ///
    /// See notes from the `backtrace` crate about when this information might and might not be
    /// available.
    #[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
    pub struct StackFrame {
        /// Instruction pointer
        pub ip:   *mut raw::c_void,
        /// Function name if found
        pub name: Option<String>,
        /// Address
        pub addr: Option<*mut raw::c_void>,
        /// Source file name
        pub file: Option<String>,
        /// Source line number
        pub line: Option<u32>,
    }

    impl fmt::Debug for StackFrame {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let n = self.line.map_or(Cow::Borrowed("<unknown>"), |n| Cow::Owned(format!("{}", n)));

            write!(f, "{:16p} - {} ({}:{})",
                self.ip,
                self.name.as_ref().map(Deref::deref).unwrap_or("<unknown>"),
                self.file.as_ref().map(Deref::deref).unwrap_or("<unknown>"),
                n,
                )
        }
    }
}

#[cfg(not(any(feature="debug_print_trace", feature="backtrace")))]
mod trace {
    use std::fmt::{self, Debug};
    use std::ops::{Deref, DerefMut};

    #[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
    pub struct Trace<T>(T);

    impl<T> Trace<T> {
        #[inline(always)]
        pub fn new(t: T) -> Self { Trace(t) }

        #[inline(always)]
        pub fn unwrap(self) -> T { self.0 }
    }

    /// Enables any error to automatically be wrapped in `Trace<E>` when `Result<T, E>` is used in
    /// the `try!` macro.
    impl<T> From<T> for Trace<T> {
        #[inline(always)]
        fn from(t: T) -> Self {
            Trace::new(t)
        }
    }

    /// Implementation which only forwards to the internal type, making `Trace` invisible.
    impl<T: Debug> Debug for Trace<T> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            // Trace is just invisible here, propagate
            Debug::fmt(&self.0, f)
        }
    }

    impl<T> Deref for Trace<T> {
        type Target = T;

        fn deref(&self) -> &T {
            &self.0
        }
    }

    impl<T> DerefMut for Trace<T> {
        fn deref_mut(&mut self) -> &mut T {
            &mut self.0
        }
    }
}
