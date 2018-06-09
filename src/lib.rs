//! This crate implements an execution context for Rust.  An execution context
//! carries data through a logical flow of execution.
//!
//! This is heavily inspired by the .NET `ExecutionContext` system as an
//! experiment if such an API would make sense in Rust.
//!
//! The main differences to the system in .NET:
//!
//! *   This purely implements an execution context and not a synchronization
//!     context.  As such the implementation is simplified.
//! *   This provides the ability to permanently disable the flow propagation.
//! *   This crate refers to "ambient data" and "async locals" as "flow-local data".
//! *   Capturing always returns an execution context even if the flow is
//!     suppressed.  Consequently the run method is no longer static but
//!     stored on the instance.
//! *   This uses atomic reference counting underneath the hood instead of
//!     using a garbage collector.  This also means that performance
//!     characteristics will be different.
//! *   `FlowLocal` data have a initialization expression that is invoked if
//!     no local data is stored in the current flow.
//!
//! # Example Usage
//!
//! ```
//! #[macro_use]
//! extern crate execution_context;
//!
//! use execution_context::ExecutionContext;
//! use std::env;
//! use std::thread;
//!
//! flow_local!(static LOCALE: String = env::var("LANG").unwrap_or_else(|_| "en_US".into()));
//!
//! fn main() {
//!     println!("the current locale is {}", LOCALE.get());
//!     LOCALE.set("de_DE".into());
//!     println!("changing locale to {}", LOCALE.get());
//!
//!     let ec = ExecutionContext::capture();
//!     thread::spawn(move || {
//!         ec.run(|| {
//!             println!("the locale in the child thread is {}", LOCALE.get());
//!             LOCALE.set("fr_FR".into());
//!             println!("the new locale in the child thread is {}", LOCALE.get());
//!         });
//!     }).join().unwrap();
//!
//!     println!("the locale of the parent thread is again {}", LOCALE.get());
//! }
//! ```
//!
//! This example will give the following output assuming no default language was
//! set as environment variable (or the environment variable is `en_US`):
//!
//! ```plain
//! the current locale is en_US
//! changing locale to de_DE
//! the locale in the child thread is de_DE
//! the new locale in the child thread is fr_FR
//! the locale of the parent thread is again de_DE
//! ```
extern crate im;
#[macro_use]
extern crate lazy_static;

mod ctx;
mod data;

pub use ctx::*;
pub use data::*;
