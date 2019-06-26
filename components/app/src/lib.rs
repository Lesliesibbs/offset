#![feature(async_await, await_macro, arbitrary_self_types)]
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

mod app_conn;
mod connect;
pub mod gen;
mod identity;
mod multi_route_util;

pub use proto::file;
pub use proto::ser_string;

pub use proto::app_server::messages::{AppPermissions, NamedRelayAddress, RelayAddress};
pub use proto::funder::messages::{Commit, MultiCommit, PaymentStatus, Rate, Receipt};
pub use proto::funder::signature_buff::verify_receipt;
pub use proto::index_server::messages::NamedIndexServerAddress;
pub use proto::report::signature_buff::verify_move_token_hashed_report;

pub use self::app_conn::{AppBuyer, AppConfig, AppConn, AppReport, AppRoutes, AppSeller};

pub use self::connect::{connect, node_connect, ConnectError};
pub use self::identity::{identity_from_file, IdentityFromFileError};

// TODO: Possibly reduce what we export from report in the future?
pub mod report {
    pub use proto::report::messages::{
        AddFriendReport, ChannelInconsistentReport, ChannelStatusReport, DirectionReport,
        FriendLivenessReport, FriendReport, FriendReportMutation, FriendStatusReport, FunderReport,
        FunderReportMutateError, FunderReportMutation, FunderReportMutations, McBalanceReport,
        McRequestsStatusReport, MoveTokenHashedReport, RequestsStatusReport, ResetTermsReport,
        SentLocalRelaysReport, TcReport,
    };

    pub use proto::app_server::messages::{NodeReport, NodeReportMutation};
    pub use proto::index_client::messages::{
        AddIndexServer, IndexClientReport, IndexClientReportMutation,
    };
}

pub mod invoice {
    pub use crypto::invoice_id::{InvoiceId, INVOICE_ID_LEN};
}

pub mod payment {
    pub use crypto::payment_id::{PaymentId, PAYMENT_ID_LEN};
}

pub mod route {
    pub use super::multi_route_util::{safe_multi_route_amounts, MultiRouteChoice};
    pub use proto::funder::messages::FriendsRoute;
    pub use proto::index_server::messages::{MultiRoute, RouteCapacityRate};

}

pub use crypto::hash::{HashResult, HASH_RESULT_LEN};
pub use crypto::hash_lock::{HashedLock, PlainLock, HASHED_LOCK_LEN, PLAIN_LOCK_LEN};
pub use crypto::identity::{PublicKey, Signature, PUBLIC_KEY_LEN, SIGNATURE_LEN};
pub use crypto::rand::{RandValue, RAND_VALUE_LEN};
