//! Library for rendering Organya music files (Cave Story & friends)
//!
//! Based on the [organya.h](<https://github.com/Strultz/organya.h>) project.

#![forbid(unsafe_code)]
#![warn(
    missing_docs,
    unused_qualifications,
    redundant_imports,
    trivial_casts,
    trivial_numeric_casts,
    clippy::pedantic,
    clippy::missing_const_for_fn,
    clippy::suboptimal_flops
)]
#![allow(clippy::missing_errors_doc)]

mod player;
mod read_cursor;
mod song;
mod sound;

pub use {
    player::Player,
    song::{Channel, Event, Song},
};

/// How to interpolate samples
#[derive(Clone, Copy, Default)]
pub enum Interpolation {
    /// Don't use any interpolation method
    #[default]
    None,
    /// Use lagrange interpolation
    Lagrange,
}

/// Error that can happen when loading Organya files
#[derive(Debug)]
pub enum OrgError {
    /// Malformed Organya file
    Malformed,
    /// Input/Output error
    Io(std::io::Error),
}

impl From<std::io::Error> for OrgError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl std::fmt::Display for OrgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrgError::Malformed => f.write_str("malformed Organya file"),
            OrgError::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for OrgError {}

/// If a property (volume, pan, etc.) has this value, it is ignored
pub const PROPERTY_UNUSED: u8 = 0xFF;
