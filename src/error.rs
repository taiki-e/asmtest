// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::{
    boxed::Box,
    string::{String, ToString as _},
};
use core::fmt;

pub(crate) type Result<T, E = Error> = core::result::Result<T, E>;

#[derive(Debug)]
pub(crate) struct Error(ErrorKind);

#[derive(Debug)]
pub(crate) enum ErrorKind {
    Other(Box<str>),
    WithContext(Box<str>, Option<Box<dyn core::error::Error + Send + Sync>>),
}

impl Error {
    pub(crate) fn new(e: impl Into<ErrorKind>) -> Self {
        Self(e.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            ErrorKind::Other(e) | ErrorKind::WithContext(e, ..) => fmt::Display::fmt(e, f),
        }
    }
}

impl core::error::Error for Error {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match &self.0 {
            ErrorKind::Other(_) => None,
            ErrorKind::WithContext(_, e) => Some(&**e.as_ref()?),
        }
    }
}

impl From<String> for ErrorKind {
    fn from(s: String) -> Self {
        Self::Other(s.into_boxed_str())
    }
}

// Inspired by anyhow::Context.
pub(crate) trait Context<T, E> {
    fn with_context<C, F>(self, context: F) -> Result<T, Error>
    where
        C: fmt::Display,
        F: FnOnce() -> C;
}
impl<T, E> Context<T, E> for Result<T, E>
where
    E: core::error::Error + Send + Sync + 'static,
{
    fn with_context<C, F>(self, context: F) -> Result<T, Error>
    where
        C: fmt::Display,
        F: FnOnce() -> C,
    {
        match self {
            Ok(ok) => Ok(ok),
            Err(e) => Err(Error(ErrorKind::WithContext(
                context().to_string().into_boxed_str(),
                Some(Box::new(e)),
            ))),
        }
    }
}
impl<T> Context<T, core::convert::Infallible> for Option<T> {
    fn with_context<C, F>(self, context: F) -> Result<T, Error>
    where
        C: fmt::Display,
        F: FnOnce() -> C,
    {
        match self {
            Some(ok) => Ok(ok),
            None => {
                Err(Error(ErrorKind::WithContext(context().to_string().into_boxed_str(), None)))
            }
        }
    }
}
