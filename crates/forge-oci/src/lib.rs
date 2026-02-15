// thiserror/miette derive macros generate code that triggers false-positive unused_assignments
#![allow(unused_assignments)]

pub mod layout;
pub mod manifest;
pub mod registry;
pub mod tar_layer;
