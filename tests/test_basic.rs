#[macro_use]
extern crate execution_context;

use execution_context::ExecutionContext;
use std::thread;

#[test]
fn test_basic_propagation() {
    flow_local!(static TEST: u32 = 42);
    assert_eq!(*TEST.get(), 42);

    let ec = ExecutionContext::capture();
    TEST.set(23);
    assert_eq!(*TEST.get(), 23);
    ec.run(|| {
        assert_eq!(*TEST.get(), 42);
    });
}

#[test]
fn test_basic_propagation_to_threads() {
    flow_local!(static TEST: u32 = 42);
    assert_eq!(*TEST.get(), 42);

    let ec = ExecutionContext::capture();
    TEST.set(23);
    assert_eq!(*TEST.get(), 23);
    thread::spawn(move || {
        ec.run(|| {
            assert_eq!(*TEST.get(), 42);
            TEST.set(22);
        });
    }).join()
        .unwrap();

    assert_eq!(*TEST.get(), 23);
}

#[test]
fn test_flow_suppression() {
    flow_local!(static TEST: u32 = 42);

    {
        let _guard = ExecutionContext::suppress_flow();
        TEST.set(11111);

        assert!(ExecutionContext::is_flow_suppressed());

        let ec = ExecutionContext::capture();
        ec.run(|| {
            assert!(!ExecutionContext::is_flow_suppressed());
            assert_eq!(*TEST.get(), 42);
        });
    }

    assert!(!ExecutionContext::is_flow_suppressed());
}

#[test]
fn test_flow_disabling() {
    flow_local!(static TEST: u32 = 42);

    {
        let _guard = ExecutionContext::disable_flow();
        TEST.set(11111);

        assert!(ExecutionContext::is_flow_suppressed());

        let ec = ExecutionContext::capture();
        ec.run(|| {
            assert!(ExecutionContext::is_flow_suppressed());
            assert_eq!(*TEST.get(), 42);
        });
    }

    assert!(!ExecutionContext::is_flow_suppressed());
}

#[test]
fn test_running_default() {
    flow_local!(static TEST: u32 = 23);
    TEST.set(24);
    assert_eq!(*TEST.get(), 24);
    ExecutionContext::default().run(|| {
        assert_eq!(*TEST.get(), 23);
    });
}
