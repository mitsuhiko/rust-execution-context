#[macro_use]
extern crate execution_context;

use execution_context::ExecutionContext;
use std::env;
use std::thread;

flow_local!(static LOCALE: String = env::var("LANG").unwrap_or_else(|_| "en_US".into()));

fn main() {
    println!("the current locale is {}", LOCALE.get());
    LOCALE.set("de_DE".into());
    println!("changing locale to {}", LOCALE.get());

    let ec = ExecutionContext::capture();
    thread::spawn(move || {
        ec.run(|| {
            println!("the locale in the child thread is {}", LOCALE.get());
            LOCALE.set("fr_FR".into());
            println!("the new locale in the child thread is {}", LOCALE.get());
        });
    }).join().unwrap();

    println!("the locale of the parent thread is again {}", LOCALE.get());
}
