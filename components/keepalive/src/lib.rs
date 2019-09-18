#![crate_type = "lib"]
#![feature(arbitrary_self_types)]
#![feature(nll)]
#![feature(generators)]
#![feature(never_type)]
#![feature(unboxed_closures)]
#![deny(trivial_numeric_casts, warnings)]
#![allow(intra_doc_link_resolution_failure)]
#![allow(
    clippy::too_many_arguments,
    clippy::implicit_hasher,
    clippy::module_inception,
    clippy::new_without_default
)]

#[macro_use]
extern crate log;

#[macro_use]
extern crate common;

mod keepalive;

pub use self::keepalive::KeepAliveChannel;
