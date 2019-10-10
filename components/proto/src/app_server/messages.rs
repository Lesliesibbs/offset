use serde::{Deserialize, Serialize};

use capnp_conv::{capnp_conv, CapnpConvError, ReadCapnp, WriteCapnp};

use common::mutable_state::MutableState;

use crate::crypto::{InvoiceId, PaymentId, PublicKey, Uid};

use crate::funder::messages::{
    AckClosePayment, AddFriend, AddInvoice, CreatePayment, CreateTransaction, Currency,
    MultiCommit, ResetFriendChannel, ResponseClosePayment, SetFriendName, SetFriendRate,
    SetFriendRelays, SetFriendRemoteMaxDebt, TransactionResult,
};
use crate::index_client::messages::{
    ClientResponseRoutes, IndexClientReport, IndexClientReportMutation,
};
use crate::index_server::messages::{NamedIndexServerAddress, RequestRoutes};
use crate::net::messages::NetAddress;
use crate::report::messages::{FunderReport, FunderReportMutation};

// TODO: Move NamedRelayAddress and RelayAddress to another place in offst-proto?

#[capnp_conv(crate::common_capnp::named_relay_address)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NamedRelayAddress<B = NetAddress> {
    pub public_key: PublicKey,
    pub address: B,
    pub name: String,
}

#[capnp_conv(crate::common_capnp::relay_address)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelayAddress<B = NetAddress> {
    pub public_key: PublicKey,
    pub address: B,
}

impl<B> From<NamedRelayAddress<B>> for RelayAddress<B> {
    fn from(from: NamedRelayAddress<B>) -> Self {
        RelayAddress {
            public_key: from.public_key,
            address: from.address,
        }
    }
}

#[capnp_conv(crate::report_capnp::node_report)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeReport<B = NetAddress> {
    pub funder_report: FunderReport<B>,
    pub index_client_report: IndexClientReport<B>,
}

#[capnp_conv(crate::report_capnp::node_report_mutation)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeReportMutation<B = NetAddress>
where
    B: Clone,
{
    Funder(FunderReportMutation<B>),
    IndexClient(IndexClientReportMutation<B>),
}

#[capnp_conv(crate::app_server_capnp::report_mutations::opt_app_request_id)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptAppRequestId {
    AppRequestId(Uid),
    Empty,
}

impl From<Option<Uid>> for OptAppRequestId {
    fn from(opt: Option<Uid>) -> Self {
        match opt {
            Some(uid) => OptAppRequestId::AppRequestId(uid),
            None => OptAppRequestId::Empty,
        }
    }
}

impl From<OptAppRequestId> for Option<Uid> {
    fn from(opt: OptAppRequestId) -> Self {
        match opt {
            OptAppRequestId::AppRequestId(uid) => Some(uid),
            OptAppRequestId::Empty => None,
        }
    }
}

#[capnp_conv(crate::app_server_capnp::report_mutations)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportMutations<B = NetAddress>
where
    B: Clone,
{
    #[capnp_conv(with = OptAppRequestId)]
    pub opt_app_request_id: Option<Uid>,
    pub mutations: Vec<NodeReportMutation<B>>,
}

#[allow(clippy::large_enum_variant)]
#[capnp_conv(crate::app_server_capnp::app_server_to_app)]
#[derive(Debug, PartialEq, Eq)]
pub enum AppServerToApp<B = NetAddress>
where
    B: Clone,
{
    /// Funds:
    TransactionResult(TransactionResult),
    ResponseClosePayment(ResponseClosePayment),
    /// Reports about current state:
    Report(NodeReport<B>),
    ReportMutations(ReportMutations<B>),
    ResponseRoutes(ClientResponseRoutes),
}

#[derive(Debug, PartialEq, Eq)]
pub enum NamedRelaysMutation<B = NetAddress> {
    AddRelay(NamedRelayAddress<B>),
    RemoveRelay(PublicKey),
}

#[capnp_conv(crate::app_server_capnp::open_friend)]
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct OpenFriend {
    pub friend_public_key: PublicKey,
    pub currency: Currency,
}

#[capnp_conv(crate::app_server_capnp::close_friend)]
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CloseFriend {
    pub friend_public_key: PublicKey,
    pub currency: Currency,
}

#[capnp_conv(crate::app_server_capnp::app_request)]
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum AppRequest<B = NetAddress> {
    /// Manage locally used relays:
    AddRelay(NamedRelayAddress<B>),
    RemoveRelay(PublicKey),
    /// Friend management:
    AddFriend(AddFriend<B>),
    SetFriendRelays(SetFriendRelays<B>),
    SetFriendName(SetFriendName),
    RemoveFriend(PublicKey),
    EnableFriend(PublicKey),
    DisableFriend(PublicKey),
    OpenFriend(OpenFriend),
    CloseFriend(CloseFriend),
    SetFriendRemoteMaxDebt(SetFriendRemoteMaxDebt),
    SetFriendRate(SetFriendRate),
    ResetFriendChannel(ResetFriendChannel),
    /// Buyer:
    CreatePayment(CreatePayment),
    CreateTransaction(CreateTransaction),
    RequestClosePayment(PaymentId),
    AckClosePayment(AckClosePayment),
    /// Seller:
    AddInvoice(AddInvoice),
    CancelInvoice(InvoiceId),
    CommitInvoice(MultiCommit),
    /// Request routes from one node to another:
    RequestRoutes(RequestRoutes),
    /// Manage index servers:
    AddIndexServer(NamedIndexServerAddress<B>),
    RemoveIndexServer(PublicKey),
}
#[capnp_conv(crate::app_server_capnp::app_to_app_server)]
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct AppToAppServer<B = NetAddress> {
    pub app_request_id: Uid,
    pub app_request: AppRequest<B>,
}

impl<B> AppToAppServer<B> {
    pub fn new(app_request_id: Uid, app_request: AppRequest<B>) -> Self {
        AppToAppServer {
            app_request_id,
            app_request,
        }
    }
}

// TODO: Move this code to a separate module:

#[derive(Debug)]
pub struct NodeReportMutateError;

impl<B> NodeReport<B>
where
    B: Eq + Clone,
{
    pub fn mutate(
        &mut self,
        mutation: &NodeReportMutation<B>,
    ) -> Result<(), NodeReportMutateError> {
        match mutation {
            NodeReportMutation::<B>::Funder(mutation) => self
                .funder_report
                .mutate(mutation)
                .map_err(|_| NodeReportMutateError)?,
            NodeReportMutation::<B>::IndexClient(mutation) => {
                self.index_client_report.mutate(mutation)
            }
        };
        Ok(())
    }
}

impl<B> MutableState for NodeReport<B>
where
    B: Eq + Clone,
{
    type Mutation = NodeReportMutation<B>;
    type MutateError = NodeReportMutateError;

    fn mutate(&mut self, mutation: &NodeReportMutation<B>) -> Result<(), NodeReportMutateError> {
        self.mutate(mutation)
    }
}

#[capnp_conv(crate::app_server_capnp::app_permissions)]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppPermissions {
    /// Can request routes
    pub routes: bool,
    /// Can send credits as a buyer
    pub buyer: bool,
    /// Can receive credits as a seller
    pub seller: bool,
    /// Can configure friends
    pub config: bool,
}
