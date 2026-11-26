#[cfg(feature = "count-work")]
thread_local! {
    static STEPS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

#[inline(always)]
pub(crate) fn tick(steps: usize) {
    #[cfg(feature = "count-work")]
    STEPS.with(|s| s.set(s.get() + steps as u64));
    #[cfg(not(feature = "count-work"))]
    let _ = steps;
}

#[cfg(feature = "count-work")]
pub fn count<R>(f: impl FnOnce() -> R) -> (R, u64) {
    let before = STEPS.with(std::cell::Cell::get);
    let r = f();
    (r, STEPS.with(std::cell::Cell::get) - before)
}
