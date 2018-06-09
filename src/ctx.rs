use std::any::TypeId;
use std::cell::UnsafeCell;
use std::fmt;
use std::panic;
use std::rc::Rc;
use std::sync::Arc;

use data::{LocalMap, Opaque};

lazy_static! {
    static ref DEFAULT_ACTIVE_CONTEXT: Arc<ExecutionContextImpl> = Arc::new(ExecutionContextImpl {
        flow_propagation: FlowPropagation::Active,
        locals: Default::default(),
    });
    static ref DEFAULT_DISABLED_CONTEXT: Arc<ExecutionContextImpl> = Arc::new(ExecutionContextImpl {
        flow_propagation: FlowPropagation::Disabled,
        locals: Default::default(),
    });
}

thread_local! {
    // we are using an unsafe cell here because current context is held
    // in thread local storage only and as such there can never be
    // references to it accessed from multiple places.
    static CURRENT_CONTEXT: UnsafeCell<Arc<ExecutionContextImpl>> =
        UnsafeCell::new(DEFAULT_ACTIVE_CONTEXT.clone());
}

#[derive(PartialEq, Debug, Copy, Clone)]
enum FlowPropagation {
    Active,
    Suppressed,
    Disabled,
}

#[derive(Clone)]
pub(crate) struct ExecutionContextImpl {
    flow_propagation: FlowPropagation,
    locals: LocalMap,
}

impl ExecutionContextImpl {
    /// Wraps the execution context implementation in an Arc.
    ///
    /// Ths optimizes the two well known default cases.
    fn into_arc(self) -> Arc<ExecutionContextImpl> {
        match (self.flow_propagation, self.locals.is_empty()) {
            (FlowPropagation::Active, true) => DEFAULT_ACTIVE_CONTEXT.clone(),
            (FlowPropagation::Disabled, true) => DEFAULT_DISABLED_CONTEXT.clone(),
            _ => Arc::new(self),
        }
    }

    fn has_active_flow(&self) -> bool {
        self.flow_propagation == FlowPropagation::Active
    }
}

/// An execution context is a container for the current logical flow of execution.
///
/// This container holds all state that needs to be carried forward with the logical thread
/// of execution.
///
/// The ExecutionContext class provides the functionality to capture and transfer the
/// encapsulated context across asynchronous points such as threads or tasks.
///
/// An execution context can be captured, send and cloned.  This permits a context to be
/// carried to other threads.  The default execution context can be acquired at any
/// by calling `ExecutionContext::default`.
#[derive(Clone)]
pub struct ExecutionContext {
    inner: Arc<ExecutionContextImpl>,
}

impl fmt::Debug for ExecutionContext {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ExecutionContext").finish()
    }
}

impl Default for ExecutionContext {
    fn default() -> ExecutionContext {
        ExecutionContext {
            inner: DEFAULT_ACTIVE_CONTEXT.clone(),
        }
    }
}

/// A guard for suspended flows.
///
/// This object is used as a guard to resume the flow that was suppressed by a
/// call to `ExecutionContext::suppress_flow` or `ExecutionContext::disable_flow`.
/// When it is dropped the flow is resumed.
///
/// The guard is internally reference counted.
// the Rc is to make it non send
#[derive(Clone)]
pub struct FlowGuard(Rc<FlowPropagation>);

impl ExecutionContext {
    /// Captures the current execution context and returns it.
    ///
    /// If the current execution context is suppressed then this will instead
    /// capture an empty default scope.  Capturing will always succeed.
    ///
    /// Capturing the execution context means that the flow of data will
    /// branch off here.  If a flow local is modified after the flow is
    /// captured it will not be reflected in the captured context.
    ///
    /// ## Example
    ///
    /// ```
    /// # use execution_context::ExecutionContext;
    /// let ec = ExecutionContext::capture();
    /// ec.run(|| {
    ///     // this code runs in the flow of the given execution context.
    /// });
    /// ```
    pub fn capture() -> ExecutionContext {
        ExecutionContext {
            inner: CURRENT_CONTEXT.with(|ctx| unsafe {
                let current = ctx.get();
                match (*current).flow_propagation {
                    FlowPropagation::Active => (*current).clone(),
                    FlowPropagation::Suppressed => DEFAULT_ACTIVE_CONTEXT.clone(),
                    FlowPropagation::Disabled => DEFAULT_DISABLED_CONTEXT.clone(),
                }
            }),
        }
    }

    /// Suppresses the flow.
    ///
    /// This returns a clonable non-send guard that when dropped restores the
    /// flow.  This can be used to spawn an operation that should not be considered
    /// to be part of the same logical flow.  Once a new execution context has been
    /// created, that context will start its own flow again.
    ///
    /// To permanently disable flow propagation use `disable_flow`.
    ///
    /// ## Example
    ///
    /// ```
    /// # use execution_context::ExecutionContext;
    /// {
    ///     let _guard = ExecutionContext::suppress_flow();
    ///     let ec = ExecutionContext::capture();
    ///     ec.run(|| {
    ///         // a new flow is started here because the captured flow was
    ///         // suppressed.
    ///     });
    /// }
    /// // the flow is resumed here
    /// ```
    pub fn suppress_flow() -> FlowGuard {
        FlowGuard(Rc::new(ExecutionContext::set_flow_propagation(
            FlowPropagation::Suppressed,
        )))
    }

    /// Permanently disables the flow.
    ///
    /// This works similar to `suppress_flow` but instead of just starting a new
    /// flow this permanently disables the flow.  The flow can be manually restored
    /// by a call to `restore_flow`.
    pub fn disable_flow() -> FlowGuard {
        FlowGuard(Rc::new(ExecutionContext::set_flow_propagation(
            FlowPropagation::Disabled,
        )))
    }

    /// Restores the flow.
    ///
    /// In normal situations the flow is restored when the flow guard is
    /// dropped.  However when for instance the flow is permanently disabled
    /// with `disable_flow` new branches will never have their flow restored.
    /// In those situations it might be useful to call into this function to
    /// restore the flow.
    pub fn restore_flow() {
        ExecutionContext::set_flow_propagation(FlowPropagation::Active);
    }

    /// Checks if the flow is currently suppressed.
    ///
    /// A caller cannot determine if the flow is just temporarily suppressed
    /// or permanently disabled.
    pub fn is_flow_suppressed() -> bool {
        CURRENT_CONTEXT.with(|ctx| unsafe { !(*ctx.get()).has_active_flow() })
    }

    /// Runs a function in the context of the given execution context.
    ///
    /// The captured execution flow will be carried forward.  If the flow
    /// was suppressed a new flow is started.  In case the flow was disabled
    /// then it's also disabled here.
    ///
    /// ## Example
    ///
    /// ```
    /// # use std::thread;
    /// # use execution_context::ExecutionContext;
    /// let ec = ExecutionContext::capture();
    /// thread::spawn(move || {
    ///     ec.run(|| {
    ///         // the captured execution context is carried into
    ///         // another thread.
    ///     });
    /// });
    /// ```
    pub fn run<F: FnOnce() -> R, R>(&self, f: F) -> R {
        // figure out where we want to switch to.  In case the current
        // flow is the target flow, we can get away without having to do
        // any panic handling and pointer swapping.  Otherwise this block
        // switches us to the new context and returns the old one.
        let did_switch = CURRENT_CONTEXT.with(|ctx| unsafe {
            let ptr = ctx.get();
            if &**ptr as *const _ == &*self.inner as *const _ {
                None
            } else {
                let old = (*ptr).clone();
                *ptr = self.inner.clone();
                Some(old)
            }
        });

        match did_switch {
            None => {
                // None means no switch happened.  We can invoke the function
                // just like that, no changes necessary.
                f()
            }
            Some(old_ctx) => {
                // this is for the case where we just switched the execution
                // context.  This means we need to catch the panic, restore the
                // old context and resume the panic if needed.
                let rv = panic::catch_unwind(panic::AssertUnwindSafe(|| f()));
                CURRENT_CONTEXT.with(|ctx| unsafe { *ctx.get() = old_ctx });
                match rv {
                    Err(err) => panic::resume_unwind(err),
                    Ok(rv) => rv,
                }
            }
        }
    }

    /// Internal helper to update the flow propagation.
    fn set_flow_propagation(new: FlowPropagation) -> FlowPropagation {
        CURRENT_CONTEXT.with(|ctx| unsafe {
            let ptr = ctx.get();
            let old = (*ptr).flow_propagation;
            if old != new {
                *ptr = ExecutionContextImpl {
                    flow_propagation: new,
                    locals: (*ptr).locals.clone(),
                }.into_arc();
            }
            old
        })
    }

    /// Inserts a value into the locals.
    pub(crate) fn set_local_value(key: TypeId, value: Arc<Box<Opaque>>) {
        CURRENT_CONTEXT.with(|ctx| unsafe {
            let ptr = ctx.get();
            let locals = (*ptr).locals.insert(key, value);
            *ptr = ExecutionContextImpl {
                flow_propagation: (*ptr).flow_propagation,
                locals: locals,
            }.into_arc();
        });
    }

    /// Returns a value from the locals.
    pub(crate) fn get_local_value(key: TypeId) -> Option<Arc<Box<Opaque>>> {
        CURRENT_CONTEXT.with(|ctx| unsafe { (*ctx.get()).locals.get(&key) })
    }
}

impl Drop for FlowGuard {
    fn drop(&mut self) {
        if let Some(old) = Rc::get_mut(&mut self.0) {
            ExecutionContext::set_flow_propagation(*old);
        }
    }
}
