use std::{
    pin::Pin,
    task::{Context, Poll},
};

use reqwest::Url;

pub trait UrlExt {
    fn extend_path(&mut self, segments: impl IntoIterator<Item = impl AsRef<str>>);
}

impl UrlExt for Url {
    fn extend_path(&mut self, segments: impl IntoIterator<Item = impl AsRef<str>>) {
        let mut editor = self
            .path_segments_mut()
            .expect("URL should be a valid base");
        editor.extend(segments);
    }
}

/// A set of futures which can be awaited in a loop
///
pub struct FutureSet<F, Output, Meta>
where
    F: Future<Output = Output> + Unpin,
    Meta: Unpin,
{
    inner: Vec<(F, Meta)>,
}

#[allow(clippy::missing_const_for_fn)] // nobody's gonna use this in const
impl<F, Output, Meta> FutureSet<F, Output, Meta>
where
    F: Future<Output = Output> + Unpin,
    Meta: Unpin,
{
    /// Construct a new `FutureSet` from a `Vec` of futures
    #[must_use]
    pub fn new(futures: Vec<(F, Meta)>) -> Self {
        Self { inner: futures }
    }

    /// Await all the futures in the set and return the result of the next one that completes.
    ///
    /// Returns `None` if there are no more futures left in the set.
    #[must_use]
    pub async fn next(&mut self) -> Option<(Output, Meta)> {
        self.await
    }

    /// Check if the set is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns the number of futures left in the set
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<F, Output, Meta> Future for FutureSet<F, Output, Meta>
where
    F: Future<Output = Output> + Unpin,
    Meta: Unpin,
{
    type Output = Option<(Output, Meta)>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // if we've got no more futures left, return none immediately
        if self.inner.is_empty() {
            return Poll::Ready(None);
        }
        // try polling each future
        for (idx, (future, ..)) in self.inner.iter_mut().enumerate() {
            if let Poll::Ready(res) = Pin::new(future).poll(cx) {
                // remove from list and return
                let removed = self.inner.swap_remove(idx);
                return Poll::Ready(Some((res, removed.1)));
            }
        }
        // none are ready, but we've scheduled each to wake the thread back up
        Poll::Pending
    }
}

impl<F, Output, Meta> FromIterator<(F, Meta)> for FutureSet<F, Output, Meta>
where
    F: Future<Output = Output> + Unpin,
    Meta: Unpin,
{
    fn from_iter<I: IntoIterator<Item = (F, Meta)>>(iter: I) -> Self {
        Self::new(iter.into_iter().collect())
    }
}

impl<F, Output> FromIterator<F> for FutureSet<F, Output, ()>
where
    F: Future<Output = Output> + Unpin,
{
    fn from_iter<I: IntoIterator<Item = F>>(iter: I) -> Self {
        Self::new(iter.into_iter().map(|f| (f, ())).collect())
    }
}
