use crate::Diagnostic;
use std::any::{type_name, Any, TypeId};
use std::collections::HashMap;

pub trait DiagnosticBuilder {
    fn build(&self, ctx: &GatheredContext<'_>) -> Diagnostic;
}

pub trait DiagnosticContext: Any {}

pub struct GatheredContext<'a> {
    inner: HashMap<TypeId, &'a dyn DiagnosticContext>,
}

impl<'a> GatheredContext<'a> {
    pub fn new() -> Self {
        Self { inner: HashMap::new() }
    }

    pub fn add(&mut self, ctx: &'a dyn DiagnosticContext) {
        let id = ctx.type_id();
        if self.inner.insert(id, ctx).is_some() {
            panic!("the same type was added twice to the context");
        }
    }

    pub fn has<T: DiagnosticContext>(&self) -> bool {
        self.inner.contains_key(&TypeId::of::<T>())
    }

    pub fn maybe<T: DiagnosticContext>(&self) -> Option<&T> {
        let id = TypeId::of::<T>();
        let any = *self.inner.get(&id)? as &dyn Any;
        Some(any.downcast_ref().expect("we already checked the type"))
    }

    pub fn required<T: DiagnosticContext>(&self) -> &T {
        match self.maybe::<T>() {
            Some(value) => value,
            None => panic!("missing required context {}", type_name::<T>()),
        }
    }
}
