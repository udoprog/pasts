// Pasts
//
// Copyright (c) 2019-2020 Jeron Aldaron Lau
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// https://apache.org/licenses/LICENSE-2.0>, or the Zlib License, <LICENSE-ZLIB
// or http://opensource.org/licenses/Zlib>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use core::{future::Future, pin::Pin, task::Context, task::Poll};

pub enum SelectFuture<'b, T, A: Future<Output = T>> {
    //Future(&'b mut [&'a mut dyn Future<Output = T>]),
    //OptFuture(&'b mut [Option<&'a mut dyn Future<Output = T>>]),
    Future(&'b mut [A]),
    OptFuture(&'b mut [Option<A>]),
}

impl<T, A: Future<Output = T>> core::fmt::Debug for SelectFuture<'_, T, A> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Future(_) => write!(f, "Future"),
            Self::OptFuture(_) => write!(f, "OptFuture"),
        }
    }
}

impl<T, A: Future<Output = T>> Future for SelectFuture<'_, T, A> {
    type Output = (usize, T);

    // unsafe: This let's this future create `Pin`s from the slices it has a
    // unique reference to.  This is safe because `SelectFuture` never calls
    // `mem::swap()` and when `SelectFuture` drops it's no longer necessary
    // that the memory remain pinned because it's not being polled anymore.
    #[allow(unsafe_code)]
    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        let mut task_id = 0;
        match *self {
            SelectFuture::Future(ref mut tasks) => {
                for task in tasks.iter_mut().map(|task| {
                    let mut pin_fut =
                        unsafe { Pin::new_unchecked(std::ptr::read(&task)) };
                    let ret = pin_fut.as_mut().poll(cx);
                    std::mem::forget(pin_fut);
                    ret
                }) {
                    match task {
                        Poll::Ready(ret) => return Poll::Ready((task_id, ret)),
                        Poll::Pending => {}
                    }
                    task_id += 1;
                }
            }
            SelectFuture::OptFuture(ref mut tasks) => {
                for task_mut in tasks.iter_mut() {
                    if let Some(ref mut task) = task_mut {
                        let mut pin_fut = unsafe {
                            Pin::new_unchecked(std::ptr::read(&task))
                        };
                        let task = pin_fut.as_mut().poll(cx);
                        std::mem::forget(pin_fut);
                        match task {
                            Poll::Ready(ret) => {
                                *task_mut = None;
                                return Poll::Ready((task_id, ret));
                            }
                            Poll::Pending => {}
                        }
                        task_id += 1;
                    }
                }
            }
        };
        Poll::Pending
    }
}

/// A trait to select on a slice of futures (or boxed futures).
///
/// # Select on slice of futures.
/// ```
/// use pasts::prelude::*;
///
/// use core::future::Future;
/// use core::pin::Pin;
///
/// async fn async_main() {
///     let mut hello = async { "Hello" };
///     let mut world = async { "World!" };
///     // Hello is ready, so returns with index and result.
///     assert_eq!((0, "Hello"), [hello.dyn_fut(), world.dyn_fut()].select().await);
/// }
///
/// pasts::ThreadInterrupt::block_on(async_main());
/// ```
pub trait Select<T, A: Future<Output = T>> {
    /// Poll multiple futures, and return the future that's ready first.
    fn select(&mut self) -> SelectFuture<'_, T, A>;
}

impl<T, A: Future<Output = T>> Select<T, A> for [A] {
    fn select(&mut self) -> SelectFuture<'_, T, A> {
        SelectFuture::Future(self)
    }
}

impl<T, A: Future<Output = T>> Select<T, A> for [Option<A>] {
    fn select(&mut self) -> SelectFuture<'_, T, A> {
        SelectFuture::OptFuture(self)
    }
}

/// A wrapper around a `Future` trait object.
pub struct DynFuture<'a, T>(&'a mut dyn Future<Output = T>);

impl<T> core::fmt::Debug for DynFuture<'_, T> {
    fn fmt(
        &self,
        f: &mut core::fmt::Formatter<'_>,
    ) -> Result<(), core::fmt::Error> {
        write!(f, "DynFuture")
    }
}

impl<T> Future for DynFuture<'_, T> {
    type Output = T;

    #[allow(unsafe_code)]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut pin_fut =
            unsafe { Pin::new_unchecked(std::ptr::read(&self.0)) };
        let ret = pin_fut.as_mut().poll(cx);
        std::mem::forget(pin_fut);
        ret
    }
}

/// Trait for converting `Future`s to pinned trait objects.
pub trait DynFut<'a, T> {
    /// Get a trait object from a future.
    fn dyn_fut(&'a mut self) -> DynFuture<'a, T>;
}

impl<'a, T, F> DynFut<'a, T> for F
where
    F: Future<Output = T>,
{
    fn dyn_fut(&'a mut self) -> DynFuture<'a, T> {
        DynFuture(self)
    }
}
