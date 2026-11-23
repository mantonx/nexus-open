//! Rendering and layout model for the Corsair iCUE Nexus touch strip.
//!
//! Everything here is pure: no USB, no sockets, no threads. Given fonts, a
//! theme and the data to show, it produces a 640×48 frame. That keeps the
//! whole visual layer testable on the simulator without hardware.

pub mod art;
pub mod canvas;
pub mod card;
pub mod config;
pub mod demo;
pub(crate) mod draw;
pub mod engine;
pub mod fonts;
pub mod forecast;
pub mod gesture;
pub mod icons;
pub mod layout;
pub mod scenes;
pub mod signal;
pub mod text;
pub mod theme;
pub mod widget;

/// Width of the Nexus panel in pixels.
pub const WIDTH: u32 = 640;
/// Height of the Nexus panel in pixels.
pub const HEIGHT: u32 = 48;
