use common::canonical_serialize::CanonicalSerialize;
use common::mutable_state::MutableState;
use crypto::identity::PublicKey;

use crate::funder::messages::{UserRequestSendFunds, ResponseReceived,
                            ReceiptAck, AddFriend, SetFriendAddress, 
                            SetFriendName, SetFriendRemoteMaxDebt, ResetFriendChannel};
use crate::report::messages::{FunderReport, FunderReportMutation};
use crate::index_client::messages::{IndexClientReport, 
    IndexClientReportMutation, ClientResponseRoutes};
use crate::index_server::messages::RequestRoutes;
use crate::net::messages::NetAddress;

use index_client::messages::AddIndexServer;

// TODO: Move NamedRelayAddress and RelayAddress to another place in offst-proto?
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NamedRelayAddress<B=NetAddress> {
    pub public_key: PublicKey,
    pub address: B,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelayAddress<B=NetAddress> {
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

impl<B> CanonicalSerialize for RelayAddress<B> 
where
    B: CanonicalSerialize,
{
    fn canonical_serialize(&self) -> Vec<u8> {
        let mut res_bytes = Vec::new();
        res_bytes.extend_from_slice(&self.public_key);
        res_bytes.extend_from_slice(&self.address.canonical_serialize());
        res_bytes
    }
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeReport<B=NetAddress> 
where   
    B: Clone,
{
    pub funder_report: FunderReport<B>,
    pub index_client_report: IndexClientReport<B>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeReportMutation<B=NetAddress> 
where
    B: Clone,
{
    Funder(FunderReportMutation<B>),
    IndexClient(IndexClientReportMutation<B>),
}

#[derive(Debug, PartialEq, Eq)]
pub enum AppServerToApp<B=NetAddress> 
where
    B: Clone,
{
    /// Funds:
    ResponseReceived(ResponseReceived),
    /// Reports about current state:
    Report(NodeReport<B>),
    ReportMutations(Vec<NodeReportMutation<B>>),
    ResponseRoutes(ClientResponseRoutes),
}

pub enum NamedRelaysMutation<B=NetAddress> {
    AddRelay(NamedRelayAddress<B>),
    RemoveRelay(PublicKey),
}

#[derive(Debug, PartialEq, Eq)]
pub enum AppToAppServer<B=NetAddress> {
    /// Manage locally used relays:
    AddRelay(NamedRelayAddress<B>),
    RemoveRelay(PublicKey),
    /// Sending funds:
    RequestSendFunds(UserRequestSendFunds),
    ReceiptAck(ReceiptAck),
    /// Friend management:
    AddFriend(AddFriend<B>),
    SetFriendRelays(SetFriendAddress<B>),
    SetFriendName(SetFriendName),
    RemoveFriend(PublicKey),
    EnableFriend(PublicKey),
    DisableFriend(PublicKey),
    OpenFriend(PublicKey),
    CloseFriend(PublicKey),
    SetFriendRemoteMaxDebt(SetFriendRemoteMaxDebt),
    ResetFriendChannel(ResetFriendChannel),
    /// Request routes from one node to another:
    RequestRoutes(RequestRoutes),
    /// Manage index servers:
    AddIndexServer(AddIndexServer<B>),
    RemoveIndexServer(PublicKey),
}


#[derive(Debug)]
pub struct NodeReportMutateError;


impl<B> MutableState for NodeReport<B>
where
    B: Eq + Clone,
{
    type Mutation = NodeReportMutation<B>;
    type MutateError = NodeReportMutateError;

    fn mutate(&mut self, mutation: &NodeReportMutation<B>)
        -> Result<(), NodeReportMutateError> {

        match mutation {
            NodeReportMutation::Funder(mutation) => 
                self.funder_report.mutate(mutation)
                    .map_err(|_| NodeReportMutateError)?,
            NodeReportMutation::IndexClient(mutation) => 
                self.index_client_report.mutate(mutation),
        };
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppPermissions {
    /// Receives reports about state
    pub reports: bool,
    /// Can request routes
    pub routes: bool,
    /// Can send credits
    pub send_funds: bool,
    /// Can configure friends
    pub config: bool,
}
