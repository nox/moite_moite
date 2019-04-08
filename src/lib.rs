// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! > « On fait moite-moite ? »
//!
//! This crate offers a mechanism to split a value into two owned parts.
//!
//! See the [`sync::split`](sync/fn.split.html) function for more details.

pub mod sync;

/// Splits a value into two mutable references.
///
/// See the [`sync::split`](sync/fn.split.html) function for more details.
///
/// # Example
///
/// ```rust
/// use moite_moite::SplitMut;
///
/// let mut value = ("baguette", "délicieux");
/// {
///     let (left, right) = value.split_mut();
///     *right = "exquis";
///
///     assert_eq!(*left, "baguette");
///     assert_eq!(*right, "exquis");
/// }
/// assert_eq!(value, ("baguette", "exquis"));
/// ```
pub trait SplitMut<L, R>
where
    L: ?Sized,
    R: ?Sized,
{
    fn split_mut(&mut self) -> (&mut L, &mut R);
}

impl<L, R> SplitMut<L, R> for (L, R) {
    #[inline]
    fn split_mut(&mut self) -> (&mut L, &mut R) {
        let (ref mut left, ref mut right) = *self;
        (left, right)
    }
}
