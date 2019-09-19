#![feature(arbitrary_self_types)]
#![feature(nll)]
#![feature(generators)]
#![feature(never_type)]
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

use std::io;
use structopt::StructOpt;

use stctrl::stctrllib::{stctrl, StCtrlCmd, StCtrlError};

fn run() -> Result<(), StCtrlError> {
    env_logger::init();
    let st_ctrl_cmd = StCtrlCmd::from_args();
    stctrl(st_ctrl_cmd, &mut io::stdout())
}

fn main() {
    if let Err(e) = run() {
        error!("error: {:?}", e);
    }
}
