use plinky_diagnostics::ObjectSpan;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

pub(crate) fn intern<T: Internable>(value: impl Into<T>) -> Interned<T> {
    T::interner().intern(value.into())
}

pub(crate) struct Interner<T: Internable> {
    state: Mutex<InternerState<T>>,
}

impl<T: Internable> Interner<T> {
    const fn new() -> Self {
        Self { state: Mutex::new(InternerState { data: Vec::new(), mapping: BTreeMap::new() }) }
    }

    fn intern(&self, value: T) -> Interned<T> {
        let mut state = self.state.lock().expect("poisoned interner");
        if let Some(idx) = state.mapping.get(&value) {
            Interned(*idx, PhantomData)
        } else {
            let idx = state.data.len();
            state.data.push(Arc::new(value.clone()));
            state.mapping.insert(value, idx);
            Interned(idx, PhantomData)
        }
    }

    fn resolve(&self, interned: Interned<T>) -> Arc<T> {
        let state = self.state.lock().expect("poisoned interner");
        state.data[interned.0].clone()
    }
}

struct InternerState<T: Internable> {
    data: Vec<Arc<T>>,
    mapping: BTreeMap<T, usize>,
}

pub(crate) struct Interned<T: Internable>(usize, PhantomData<T>);

impl<T: Internable> Interned<T> {
    pub(crate) fn resolve(&self) -> Arc<T> {
        T::interner().resolve(*self)
    }
}

impl<T: Internable> Copy for Interned<T> {}

impl<T: Internable> Clone for Interned<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Internable> PartialEq for Interned<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T: Internable> Eq for Interned<T> {}

impl<T: Internable + PartialOrd> PartialOrd for Interned<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Internable + Ord> Ord for Interned<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.resolve().cmp(&other.resolve())
    }
}

impl<T: Internable> Hash for Interned<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<T: Internable + Debug> Debug for Interned<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.resolve(), f)
    }
}

impl<T: Internable + Display> Display for Interned<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.resolve(), f)
    }
}

pub(crate) trait Internable: Clone + Eq + Ord + 'static {
    fn interner() -> &'static Interner<Self>;
}

impl Internable for String {
    fn interner() -> &'static Interner<Self> {
        static INTERNER: Interner<String> = Interner::new();
        &INTERNER
    }
}

impl Internable for ObjectSpan {
    fn interner() -> &'static Interner<Self> {
        static INTERNER: Interner<ObjectSpan> = Interner::new();
        &INTERNER
    }
}
