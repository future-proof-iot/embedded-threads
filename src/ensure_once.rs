use super::arch::{interrupt, Mutex};
use core::cell::{Ref, RefCell, RefMut};

pub(crate) struct EnsureOnce<T> {
    inner: Mutex<RefCell<T>>,
}

impl<T> EnsureOnce<T> {
    pub const fn new(inner: T) -> Self {
        Self {
            inner: Mutex::new(RefCell::new(inner)),
        }
    }

    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(Ref<T>) -> R,
    {
        interrupt::free(|cs| self.with_cs(cs, f))
    }
    pub fn with_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(RefMut<T>) -> R,
    {
        interrupt::free(|cs| self.with_mut_cs(cs, f))
    }

    pub fn with_cs<F, R>(&self, cs: &interrupt::CriticalSection, f: F) -> R
    where
        F: FnOnce(Ref<T>) -> R,
    {
        f(self.inner.borrow(cs).borrow())
    }

    pub fn with_mut_cs<F, R>(&self, cs: &interrupt::CriticalSection, f: F) -> R
    where
        F: FnOnce(RefMut<T>) -> R,
    {
        f(self.inner.borrow(cs).borrow_mut())
    }

    // pub fn borrow_mut<'a>(&'a self, cs: &'a interrupt::CriticalSection) -> RefMut<T> {
    //     self.inner.borrow(cs).borrow_mut()
    // }
    // pub fn borrow<'a>(&'a self, cs: &'a interrupt::CriticalSection) -> Ref<T> {
    //     self.inner.borrow(cs).borrow()
    // }

    pub fn as_ptr(&self, cs: &interrupt::CriticalSection) -> *mut T {
        self.inner.borrow(cs).as_ptr()
    }
}
