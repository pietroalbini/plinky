//& Before we calculate the layout of the final elf, we need to "freeze" some parts of our data
/// structures to prevent changes that could invalidate the layout.
///
/// This marker struct can be accepted as parameter to functions that perform the mutations we want
/// to prevent, as it's possible to get references to it only before the layout is calculated.
pub(crate) struct BeforeFreeze(());

impl BeforeFreeze {
    /// # Safety
    ///
    /// Must be created only once for each linking, and must be dropped before the layout of the
    /// final object is calculated.
    pub(crate) unsafe fn new() -> Self {
        BeforeFreeze(())
    }
}
