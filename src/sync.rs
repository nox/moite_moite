// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Thread-safe version of the split mechanism.
//!
//! See the [`split`](fn.split.html) function for more details.

use std::borrow::{Borrow, BorrowMut};
use std::cell::UnsafeCell;
use std::error::Error;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::io;
use std::iter::FusedIterator;
use std::ops::{Deref, DerefMut, Drop};
use std::ptr::NonNull;
use std::sync::atomic::{self, AtomicBool, Ordering};

use SplitMut;

/// Splits a value into two owned parts.
///
/// # Sharing
///
/// The internal reference counter is atomic, allowing the left and right parts
/// to be safely shared across threads:
///
/// * `Part<T, W>` is `Sync` if `T` is `Sync`;
/// * `Part<T, W>` is `Send` if `T` is `Send` and `W` is `Send`.
///
/// There are no `W: Sync` bounds because users cannot get a reference to a
/// split `W`, whereas there is a `W: Send` bound because the dropping code
/// for `W` may be run on any thread, when its last part is dropped.
///
/// # Example
///
/// ```
/// use moite_moite::sync;
///
/// let (mut left, right) = sync::split(("hello".to_owned(), "!".to_owned()));
/// left.push_str(", world");
/// assert_eq!(format!("{}{}", left, right), "hello, world!");
/// ```
#[inline]
pub fn split<L, R, W>(value: W) -> (Part<L, W>, Part<R, W>)
where
    L: ?Sized,
    R: ?Sized,
    W: SplitMut<L, R>,
{
    let holder = Box::new(WholeCell {
        should_drop_value: AtomicBool::new(false),
        value: UnsafeCell::new(value),
    });

    let (left, right) = {
        let (left, right) = unsafe { &mut *holder.value.get() }.split_mut();
        (left as *mut _, right as *mut _)
    };
    let holder = NonNull::new(Box::into_raw(holder)).expect("never null");
    let left = Part {
        whole: WholeRef(holder),
        ptr: NonNull::new(left).expect("never null"),
    };
    let right = Part {
        whole: WholeRef(holder),
        ptr: NonNull::new(right).expect("never null"),
    };
    (left, right)
}

/// A part of a split value, itself splittable.
///
/// Mutably derefs to `T`.
///
/// See the [`split`](fn.split.html) function for more details.
pub struct Part<T: ?Sized, W: ?Sized> {
    #[allow(dead_code)]
    whole: WholeRef<W>,
    ptr: NonNull<T>,
}

impl<L, R, T, W> SplitMut<L, R> for Part<T, W>
where
    L: ?Sized,
    R: ?Sized,
    T: SplitMut<L, R> + ?Sized,
    W: ?Sized,
{
    #[inline]
    fn split_mut(&mut self) -> (&mut L, &mut R) {
        (**self).split_mut()
    }
}

#[repr(transparent)]
struct WholeRef<W: ?Sized>(NonNull<WholeCell<W>>);

struct WholeCell<W: ?Sized> {
    should_drop_value: AtomicBool,
    value: UnsafeCell<W>,
}

impl<W> Drop for WholeRef<W>
where
    W: ?Sized,
{
    #[inline]
    fn drop(&mut self) {
        unsafe {
            if self
                .0
                .as_ref()
                .should_drop_value
                .swap(true, Ordering::Release)
            {
                atomic::fence(Ordering::Acquire);
                drop(Box::from_raw(self.0.as_ptr()));
            }
        }
    }
}

unsafe impl<T, W> Send for Part<T, W>
where
    T: Send + ?Sized,
    W: Send + ?Sized,
{
}

unsafe impl<T, W> Sync for Part<T, W>
where
    T: Sync + ?Sized,
    W: ?Sized,
{
}

impl<T, W> Deref for Part<T, W>
where
    T: ?Sized,
    W: ?Sized,
{
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.as_ref() }
    }
}

impl<T, W> DerefMut for Part<T, W>
where
    T: ?Sized,
    W: ?Sized,
{
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.ptr.as_mut() }
    }
}

// Below that comment lies only boring code. Really really boring code.
// Go do something more useful, eat some bread for example.

impl<T, W> AsRef<T> for Part<T, W>
where
    T: ?Sized,
    W: ?Sized,
{
    #[inline]
    fn as_ref(&self) -> &T {
        self
    }
}

impl<T, W> AsMut<T> for Part<T, W>
where
    T: ?Sized,
    W: ?Sized,
{
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        self
    }
}

impl<T, W> Borrow<T> for Part<T, W>
where
    T: ?Sized,
    W: ?Sized,
{
    #[inline]
    fn borrow(&self) -> &T {
        self
    }
}

impl<T, W> BorrowMut<T> for Part<T, W>
where
    T: ?Sized,
    W: ?Sized,
{
    #[inline]
    fn borrow_mut(&mut self) -> &mut T {
        self
    }
}

impl<T, W> fmt::Debug for Part<T, W>
where
    T: fmt::Debug + ?Sized,
    W: ?Sized,
{
    #[inline]
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        (**self).fmt(fmt)
    }
}

impl<T, W> fmt::Display for Part<T, W>
where
    T: fmt::Display + ?Sized,
    W: ?Sized,
{
    #[inline]
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        (**self).fmt(fmt)
    }
}

impl<T, W> fmt::Pointer for Part<T, W>
where
    T: ?Sized,
    W: ?Sized,
{
    #[inline]
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        (&**self as *const T).fmt(fmt)
    }
}

impl<T, W> io::BufRead for Part<T, W>
where
    T: io::BufRead + ?Sized,
    W: ?Sized,
{
    #[inline]
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        (**self).fill_buf()
    }

    #[inline]
    fn consume(&mut self, amt: usize) {
        (**self).consume(amt)
    }

    #[inline]
    fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> io::Result<usize> {
        (**self).read_until(byte, buf)
    }

    #[inline]
    fn read_line(&mut self, buf: &mut String) -> io::Result<usize> {
        (**self).read_line(buf)
    }
}

impl<T, W> io::Read for Part<T, W>
where
    T: io::Read + ?Sized,
    W: ?Sized,
{
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (**self).read(buf)
    }

    #[inline]
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        (**self).read_to_end(buf)
    }

    #[inline]
    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        (**self).read_to_string(buf)
    }

    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        (**self).read_exact(buf)
    }
}

impl<T, W> io::Seek for Part<T, W>
where
    T: io::Seek + ?Sized,
    W: ?Sized,
{
    #[inline]
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        (**self).seek(pos)
    }
}

impl<T, W> io::Write for Part<T, W>
where
    T: io::Write + ?Sized,
    W: ?Sized,
{
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (**self).write(buf)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        (**self).flush()
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        (**self).write_all(buf)
    }

    #[inline]
    fn write_fmt(&mut self, fmt: fmt::Arguments<'_>) -> io::Result<()> {
        (**self).write_fmt(fmt)
    }
}

impl<'a, T, W> Iterator for Part<T, W>
where
    T: Iterator + ?Sized,
    W: ?Sized,
{
    type Item = T::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        (**self).next()
    }
}

impl<'a, T, W> DoubleEndedIterator for Part<T, W>
where
    T: DoubleEndedIterator + ?Sized,
    W: ?Sized,
{
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        (**self).next_back()
    }
}

impl<'a, T, W> ExactSizeIterator for Part<T, W>
where
    T: ?Sized,
    W: ?Sized,
    T: ExactSizeIterator,
{
    #[inline]
    fn len(&self) -> usize {
        (**self).len()
    }
}

impl<'a, T, W> FusedIterator for Part<T, W>
where
    T: ?Sized,
    W: ?Sized,
    T: FusedIterator,
{
}

impl<'a, T, W> Error for Part<T, W>
where
    T: ?Sized,
    W: ?Sized,
    T: Error,
{
    #[inline]
    fn description(&self) -> &str {
        (**self).description()
    }

    #[allow(deprecated)] // imagine actually deprecating stuff
    #[inline]
    fn cause(&self) -> Option<&dyn Error> {
        (**self).cause()
    }
}

impl<T, W> Hash for Part<T, W>
where
    T: ?Sized,
    W: ?Sized,
    T: Hash,
{
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        (**self).hash(state)
    }
}

impl<'a, T, W> Hasher for Part<T, W>
where
    T: ?Sized,
    W: ?Sized,
    T: Hasher,
{
    #[inline]
    fn finish(&self) -> u64 {
        (**self).finish()
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        (**self).write(bytes)
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        (**self).write_u8(i)
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        (**self).write_u16(i)
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        (**self).write_u32(i)
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        (**self).write_u64(i)
    }

    #[inline]
    fn write_u128(&mut self, i: u128) {
        (**self).write_u128(i)
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        (**self).write_usize(i)
    }

    #[inline]
    fn write_i8(&mut self, i: i8) {
        (**self).write_i8(i)
    }

    #[inline]
    fn write_i16(&mut self, i: i16) {
        (**self).write_i16(i)
    }

    #[inline]
    fn write_i32(&mut self, i: i32) {
        (**self).write_i32(i)
    }

    #[inline]
    fn write_i64(&mut self, i: i64) {
        (**self).write_i64(i)
    }

    #[inline]
    fn write_i128(&mut self, i: i128) {
        (**self).write_i128(i)
    }

    #[inline]
    fn write_isize(&mut self, i: isize) {
        (**self).write_isize(i)
    }
}
