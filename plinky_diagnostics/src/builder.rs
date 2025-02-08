use crate::Diagnostic;
use std::marker::PhantomData;

pub trait DiagnosticBuilder {
    fn build(&self, ctx: &GatheredContext<'_>) -> Diagnostic;
}

pub struct GatheredContext<'a> {
    _phantom: PhantomData<&'a u8>,
}

impl<'a> GatheredContext<'a> {
    pub fn new() -> Self {
        Self { _phantom: PhantomData }
    }
}
