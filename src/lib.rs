//!
//!

// If either the feature debug_print_trace or backtrace is set, include features for tracing:
#[cfg(any(feature="debug_print_trace", feature="backtrace"))]
extern crate backtrace;

/// Type-alias to make it easier to slot in tracing of errors:
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
        /// Creates a new stack-trace.
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
            let n = self.line.map(|n| Cow::Owned(format!("{}", n)))
                .unwrap_or(Cow::Borrowed("<unknown>"));

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
