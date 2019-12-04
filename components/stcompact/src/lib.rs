#![deny(trivial_numeric_casts, warnings)]
#![allow(intra_doc_link_resolution_failure)]
#![allow(
    clippy::too_many_arguments,
    clippy::implicit_hasher,
    clippy::module_inception,
    clippy::new_without_default
)]

#[macro_use]
extern crate common;

#[macro_use]
extern crate log;

mod convert;
mod handle_node;
mod handle_user;
mod messages;
mod permission;
#[allow(unused)]
mod persist;
mod server_init;
mod server_loop;
mod types;
