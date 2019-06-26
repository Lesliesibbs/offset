use std::fs;
use std::io;
use std::path::PathBuf;

use futures::executor::ThreadPool;

use derive_more::From;

use structopt::StructOpt;

use crate::buyer::{buyer, BuyerCmd, BuyerError};
use crate::config::{config, ConfigCmd, ConfigError};
use crate::info::{info, InfoCmd, InfoError};
use crate::seller::{seller, SellerCmd, SellerError};

use app::file::NodeAddressFile;
use app::{connect, identity_from_file};
use app::ser_string::{StringSerdeError, deserialize_from_string};

#[derive(Debug, From)]
pub enum StCtrlError {
    CreateThreadPoolError,
    // MissingIdFileArgument,
    IdFileDoesNotExist,
    // MissingNodeTicketArgument,
    NodeTicketFileDoesNotExist,
    InvalidNodeTicketFile,
    SpawnIdentityServiceError,
    ConnectionError,
    InfoError(InfoError),
    ConfigError(ConfigError),
    BuyerError(BuyerError),
    SellerError(SellerError),
    IoError(std::io::Error),
    StringSerdeError(StringSerdeError),
}

#[derive(Clone, Debug, StructOpt)]
pub enum StCtrlSubcommand {
    /// Get information about current state of node
    #[structopt(name = "info")]
    Info(InfoCmd),
    /// Configure node's state
    #[structopt(name = "config")]
    Config(ConfigCmd),
    /// Sending funds (Buyer)
    #[structopt(name = "buyer")]
    Buyer(BuyerCmd),
    /// Receiving funds (Seller)
    #[structopt(name = "seller")]
    Seller(SellerCmd),
}

/// stctrl: offST ConTRoL
/// An application used to interface with the Offst node
/// Allows to view node's state information, configure node's state and send funds to remote nodes.
#[derive(Clone, Debug, StructOpt)]
#[structopt(name = "stctrl")]
pub struct StCtrlCmd {
    /// StCtrl app identity file path
    #[structopt(parse(from_os_str), short = "I", long = "idfile")]
    pub idfile: PathBuf,
    /// Node ticket file path
    #[structopt(parse(from_os_str), short = "T", long = "ticket")]
    pub node_ticket: PathBuf,
    #[structopt(flatten)]
    pub subcommand: StCtrlSubcommand,
}

pub fn stctrl(st_ctrl_cmd: StCtrlCmd, writer: &mut impl io::Write) -> Result<(), StCtrlError> {
    let mut thread_pool = ThreadPool::new().map_err(|_| StCtrlError::CreateThreadPoolError)?;

    let StCtrlCmd {
        idfile,
        node_ticket,
        subcommand,
    } = st_ctrl_cmd;

    // Get application's identity:
    if !idfile.exists() {
        return Err(StCtrlError::IdFileDoesNotExist);
    }

    // Get node's connection information (node-ticket):
    if !node_ticket.exists() {
        return Err(StCtrlError::NodeTicketFileDoesNotExist);
    }

    let node_address_file: NodeAddressFile = deserialize_from_string(&fs::read_to_string(&node_ticket)?)?;

    // Spawn identity service:
    let app_identity_client = identity_from_file(&idfile, thread_pool.clone())
        .map_err(|_| StCtrlError::SpawnIdentityServiceError)?;

    let c_thread_pool = thread_pool.clone();
    thread_pool.run(async move {
        // Connect to node:
        let node_connection = await!(connect(
            node_address_file.public_key,
            node_address_file.address,
            app_identity_client,
            c_thread_pool.clone()
        ))
        .map_err(|_| StCtrlError::ConnectionError)?;

        match subcommand {
            StCtrlSubcommand::Info(info_cmd) => await!(info(info_cmd, node_connection, writer))?,
            StCtrlSubcommand::Config(config_cmd) => await!(config(config_cmd, node_connection))?,
            StCtrlSubcommand::Buyer(buyer_cmd) => {
                await!(buyer(buyer_cmd, node_connection, writer))?
            }
            StCtrlSubcommand::Seller(seller_cmd) => await!(seller(seller_cmd, node_connection))?,
        }
        Ok(())
    })
}
