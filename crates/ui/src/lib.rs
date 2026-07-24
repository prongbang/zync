pub mod api;

mod actions;
mod app;
mod components;
mod helpers;
mod state;

pub(crate) use actions::*;
pub(crate) use components::*;
pub(crate) use helpers::*;
pub(crate) use state::*;

pub use app::app;

pub(crate) const TAILWIND_CSS: &str = include_str!("tailwind.min.css");
pub(crate) const APP_CSS: &str = include_str!("style.css");
