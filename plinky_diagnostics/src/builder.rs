use crate::Diagnostic;
use std::any::{type_name, Any, TypeId};
use std::collections::HashMap;

pub trait DiagnosticBuilder {
    fn build(&self, ctx: &GatheredContext<'_>) -> Diagnostic;
}

pub trait DiagnosticContext: Any {}

pub struct GatheredContext<'a> {
    inner: HashMap<TypeId, ContextValue<'a>>,
}

impl<'a> GatheredContext<'a> {
    pub fn new() -> Self {
        Self { inner: HashMap::new() }
    }

    pub fn add_ref(&mut self, ctx: &'a dyn DiagnosticContext) {
        let id = ctx.type_id();
        if self.inner.insert(id, ContextValue::Ref(ctx)).is_some() {
            panic!("the same type was added twice to the context");
        }
    }

    pub fn add_owned<T: DiagnosticContext>(&mut self, ctx: T) {
        let id = ctx.type_id();
        if self.inner.insert(id, ContextValue::Box(Box::new(ctx))).is_some() {
            panic!("the same type was added twice to the context");
        }
    }

    pub fn has<T: DiagnosticContext>(&self) -> bool {
        self.inner.contains_key(&TypeId::of::<T>())
    }

    pub fn maybe<T: DiagnosticContext>(&self) -> Option<&T> {
        let id = TypeId::of::<T>();
        let any = match self.inner.get(&id)? {
            ContextValue::Ref(r) => *r as &dyn Any,
            ContextValue::Box(b) => &**b as &dyn Any,
        };
        Some(any.downcast_ref().expect("we already checked the type"))
    }

    pub fn required<T: DiagnosticContext>(&self) -> &T {
        match self.maybe::<T>() {
            Some(value) => value,
            None => panic!("missing required context {}", type_name::<T>()),
        }
    }
}

impl GatheredContext<'static> {
    pub fn remove_static_lifetime<'a>(self) -> GatheredContext<'a> {
        self
    }
}

enum ContextValue<'a> {
    Ref(&'a dyn DiagnosticContext),
    Box(Box<dyn DiagnosticContext>),
}
