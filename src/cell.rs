use core::cell::UnsafeCell;

#[derive(Default)]
#[repr(transparent)]
pub struct Cell<T>(UnsafeCell<T>);

impl<T> Cell<T> {
    /// Creates a new Cell containing the given value.
    pub fn new(value: T) -> Self {
        Self(UnsafeCell::new(value))
    }

    /// Sets the contained value.
    pub fn set(&self, value: T) {
        unsafe { *self.0.get() = value };
    }
}

impl<T: Copy> Cell<T> {
    /// Returns a copy of the contained value.
    pub fn get(&self) -> T {
        unsafe { *self.0.get() }
    }
}

impl<T: Default> Cell<T> {
    /// Takes the value of the cell, leaving Default::default() in its place.
    pub fn take(&self) -> T {
        core::mem::take(unsafe { &mut *self.0.get() })
    }
}

impl<T> Cell<Option<T>> {
    /// Returns true if the value of the cell is a [`Some`] value
    pub fn has_some(&self) -> bool {
        unsafe { (*self.0.get()).is_some() }
    }

    /// Returns true if the value of the cell is a [`None`] value
    pub fn has_none(&self) -> bool {
        unsafe { (*self.0.get()).is_none() }
    }
}

#[cfg(test)]
mod tests {
    use super::Cell;

    #[test]
    fn test_get_then_get() {
        let c = Cell::new(42);

        // Check value is correct
        assert_eq!(c.get(), 42);

        // Check value is unchanged
        assert_eq!(c.get(), 42);
    }

    #[test]
    fn test_take_then_get() {
        let c = Cell::new(42);

        assert_eq!(c.take(), 42);
        assert_eq!(c.get(), 0);
    }

    #[test]
    fn test_take_then_take() {
        let c = Cell::new(42);

        assert_eq!(c.take(), 42);
        assert_eq!(c.take(), 0);
    }

    #[test]
    fn test_set_then_get() {
        let c = Cell::new(12);

        c.set(42);

        assert_eq!(c.get(), 42);
    }

    #[test]
    fn test_has_some() {
        let c = Cell::new(Some(42));

        // We assert twice to ensure no mutation
        assert!(c.has_some());
        assert!(!c.has_none());

        assert!(c.has_some());
        assert!(!c.has_none());

        assert_eq!(c.get(), Some(42));
    }

    #[test]
    fn test_has_none() {
        let c = Cell::new(None::<()>);

        // We assert twice to ensure no mutation
        assert!(!c.has_some());
        assert!(c.has_none());

        assert!(!c.has_some());
        assert!(c.has_none());

        assert_eq!(c.get(), None);
    }
}
