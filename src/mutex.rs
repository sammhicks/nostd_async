use core::cell::UnsafeCell;

use critical_section::CriticalSection;

#[repr(transparent)]
pub struct Mutex<T>(critical_section::Mutex<UnsafeCell<T>>);

impl<T> Mutex<T> {
    /// Creates a new Cell containing the given value.
    pub const fn new(value: T) -> Self {
        Self(critical_section::Mutex::new(UnsafeCell::new(value)))
    }

    /// Sets the contained value.
    pub fn set(&self, cs: CriticalSection, value: T) {
        unsafe { *self.0.borrow(cs).get() = value };
    }
}

impl<T: Copy> Mutex<T> {
    /// Returns a copy of the contained value.
    pub fn get(&self, cs: CriticalSection) -> T {
        unsafe { *self.0.borrow(cs).get() }
    }
}

impl<T: Default> Mutex<T> {
    /// Takes the value of the cell, leaving Default::default() in its place.
    pub fn take(&self, cs: CriticalSection) -> T {
        core::mem::take(unsafe { &mut *self.0.borrow(cs).get() })
    }
}

impl<T> Mutex<Option<T>> {
    /// Returns true if the value of the cell is a [`Some`] value
    pub fn has_some(&self, cs: CriticalSection) -> bool {
        unsafe { &*self.0.borrow(cs).get() }.is_some()
    }

    /// Returns true if the value of the cell is a [`None`] value
    pub fn has_none(&self, cs: CriticalSection) -> bool {
        !self.has_some(cs)
    }
}

impl<T: Default> Default for Mutex<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

#[cfg(test)]
mod tests {
    use super::Mutex;

    #[test]
    fn test_get_then_get() {
        critical_section::with(|cs| {
            let c = Mutex::new(42);

            // Check value is correct
            assert_eq!(c.get(cs), 42);

            // Check value is unchanged
            assert_eq!(c.get(cs), 42);
        });
    }

    #[test]
    fn test_take_then_get() {
        critical_section::with(|cs| {
            let c = Mutex::new(42);

            assert_eq!(c.take(cs), 42);
            assert_eq!(c.get(cs), 0);
        });
    }

    #[test]
    fn test_take_then_take() {
        critical_section::with(|cs| {
            let c = Mutex::new(42);

            assert_eq!(c.take(cs), 42);
            assert_eq!(c.take(cs), 0);
        });
    }

    #[test]
    fn test_set_then_get() {
        critical_section::with(|cs| {
            let c = Mutex::new(12);

            c.set(cs, 42);

            assert_eq!(c.get(cs), 42);
        });
    }

    #[test]
    fn test_has_some() {
        critical_section::with(|cs| {
            let c = Mutex::new(Some(42));

            // We assert twice to ensure no mutation
            assert!(c.has_some(cs));
            assert!(!c.has_none(cs));

            assert!(c.has_some(cs));
            assert!(!c.has_none(cs));

            assert_eq!(c.get(cs), Some(42));
        });
    }

    #[test]
    fn test_has_none() {
        critical_section::with(|cs| {
            let c = Mutex::new(None::<()>);

            // We assert twice to ensure no mutation
            assert!(!c.has_some(cs));
            assert!(c.has_none(cs));

            assert!(!c.has_some(cs));
            assert!(c.has_none(cs));

            assert_eq!(c.get(cs), None);
        });
    }
}
