/// *const T but non-zero and covariant
/// Essentially a restricted version of core::ptr::NonNull<T>
#[repr(transparent)]
pub struct NonNull<T>(core::ptr::NonNull<T>);

impl<T> Clone for NonNull<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for NonNull<T> {}

impl<T> NonNull<T> {
    pub fn new(reference: &T) -> Self {
        Self(core::ptr::NonNull::from(reference))
    }

    /// Returns a shared reference to the value.
    ///
    /// # Safety
    ///
    /// When calling this method, you must ensure that the data is still valid, i.e. it hasn't been dropped
    pub unsafe fn as_ref<'a>(&self) -> &'a T {
        self.0.as_ref()
    }

    /// Acquires the underlying `*const` pointer.
    pub const fn as_ptr(self) -> *const T {
        self.0.as_ptr()
    }
}
