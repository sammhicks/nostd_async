#[cfg(feature = "cortex_m")]
pub use cortex_m::interrupt::free;

#[cfg(not(feature = "cortex_m"))]
pub fn free<F, R>(f: F) -> R
where
    F: FnOnce(&bare_metal::CriticalSection) -> R,
{
    f(&unsafe { bare_metal::CriticalSection::new() })
}
