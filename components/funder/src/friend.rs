use im::hashmap::HashMap as ImHashMap;
use im::vector::Vector as ImVec;
use std::fmt::Debug;

use signature::canonical::CanonicalSerialize;

use proto::app_server::messages::{NamedRelayAddress, RelayAddress};
use proto::crypto::PublicKey;
use proto::funder::messages::{
    CancelSendFundsOp, CollectSendFundsOp, Currency, FriendStatus, Rate, RequestSendFundsOp,
    RequestsStatus, ResetTerms, ResponseSendFundsOp,
};

use crate::token_channel::{TcMutation, TokenChannel};
use crate::types::MoveTokenHashed;

/// Any operation that goes backwards (With respect to the initial request)
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub enum BackwardsOp {
    Response(ResponseSendFundsOp),
    Cancel(CancelSendFundsOp),
    Collect(CollectSendFundsOp),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SentLocalRelays<B>
where
    B: Clone,
{
    NeverSent,
    Transition((ImVec<NamedRelayAddress<B>>, ImVec<NamedRelayAddress<B>>)), // (last sent, before last sent)
    LastSent(ImVec<NamedRelayAddress<B>>),
}

impl<B> SentLocalRelays<B>
where
    B: Clone + Debug,
{
    pub fn to_vec(&self) -> Vec<RelayAddress<B>> {
        match self {
            SentLocalRelays::NeverSent => Vec::new(),
            SentLocalRelays::Transition((last_relays, prev_last_relays)) => {
                // Create a unique list of all relay public keys:
                let mut relays: Vec<RelayAddress<B>> = Vec::new();
                for relay in last_relays {
                    relays.push(relay.clone().into());
                }
                for relay in prev_last_relays {
                    relays.push(relay.clone().into());
                }
                // Note: a vector must be sorted in order to use dedup_by_key()!
                relays.sort_by_key(|relay_address| relay_address.public_key.clone());
                relays.dedup_by_key(|relay_address| relay_address.public_key.clone());
                relays
            }
            SentLocalRelays::LastSent(last_address) => last_address
                .iter()
                .cloned()
                .map(Into::into)
                .collect::<Vec<_>>(),
        }
    }
}
#[derive(PartialEq, Eq, Clone, Serialize, Deserialize, Debug)]
pub struct ChannelInconsistent {
    pub opt_last_incoming_move_token: Option<MoveTokenHashed>,
    pub local_reset_terms: ResetTerms,
    pub opt_remote_reset_terms: Option<ResetTerms>,
}

#[derive(PartialEq, Eq, Clone, Serialize, Deserialize, Debug)]
pub struct ChannelConsistent<B> {
    /// Our mutual state with the remote side
    pub token_channel: TokenChannel<B>,
    /// A queue of requests that need to be sent to the remote friend
    pub pending_requests: ImVec<(Currency, RequestSendFundsOp)>,
    /// A queue of backwards operations (Response, Cancel, Commit) that need to be sent to the remote side
    /// We keep backwards op on a separate queue because those operations are not supposed to fail
    /// (While requests may fail due to lack of trust for example)
    pub pending_backwards_ops: ImVec<(Currency, BackwardsOp)>,
    /// Pending requests originating from the user.
    /// We care more about these requests, because those are payments that our user wants to make.
    /// This queue should be bounded in size (TODO: Check this)
    pub pending_user_requests: ImVec<(Currency, RequestSendFundsOp)>,
    /// Wanted credit frame for the remote side (Set by the user of this node)
    /// It might take a while until this value is applied, as it needs to be communicated to the
    /// remote side.
    pub wanted_remote_max_debt: ImHashMap<Currency, u128>,
    /// Can the remote friend send requests through us? This is a value chosen by the user, and it
    /// might take some time until it is applied (As it should be communicated to the remote
    /// friend).
    pub wanted_local_requests_status: ImHashMap<Currency, RequestsStatus>,
    /// Which currencies do we want to trade with this remote friend?
    /// This is our planned value for `active_currencies`. (It should be communicated to the remote
    /// friend)
    pub wanted_active_currencies: Option<Vec<Currency>>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ChannelStatus<B> {
    Inconsistent(ChannelInconsistent),
    Consistent(ChannelConsistent<B>),
}

impl<B> ChannelStatus<B>
where
    B: Clone + CanonicalSerialize,
{
    pub fn get_last_incoming_move_token_hashed(&self) -> Option<MoveTokenHashed> {
        match &self {
            ChannelStatus::Inconsistent(channel_inconsistent) => {
                channel_inconsistent.opt_last_incoming_move_token.clone()
            }
            ChannelStatus::Consistent(channel_consistent) => channel_consistent
                .token_channel
                .get_last_incoming_move_token_hashed()
                .cloned(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FriendState<B: Clone> {
    /// Public key of this node
    pub local_public_key: PublicKey,
    /// Public key of the friend node
    pub remote_public_key: PublicKey,
    /// Relays on which the friend node can be found.
    /// This list of relays corresponds to the last report of relays we got from the remote friend.
    pub remote_relays: Vec<RelayAddress<B>>,
    /// The last list of our used relays we have sent to the remote friend.
    /// We maintain this list to deal with relays drift.
    pub sent_local_relays: SentLocalRelays<B>,
    /// Locally maintained name of the remote friend node.
    pub name: String,
    /// Rate of forwarding transactions that arrived from this friend to any other friend.
    pub rate: Rate,
    /// Friend status. If disabled, we don't attempt to connect to this friend. (Friend will think
    /// we are offline).
    pub status: FriendStatus,
    /// Mutual credit channel information
    pub channel_status: ChannelStatus<B>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FriendMutation<B: Clone> {
    TcMutation(TcMutation<B>),
    SetInconsistent(ChannelInconsistent),
    SetConsistent(TokenChannel<B>),
    SetWantedRemoteMaxDebt((Currency, u128)),
    ClearWantedRemoteMaxDebt(Currency),
    SetWantedLocalRequestsStatus((Currency, RequestsStatus)),
    ClearWantedLocalRequestsStatus(Currency),
    SetWantedActiveCurrencies(Vec<Currency>),
    ClearWantedActiveCurrencies,
    PushBackPendingRequest((Currency, RequestSendFundsOp)),
    PopFrontPendingRequest,
    PushBackPendingBackwardsOp((Currency, BackwardsOp)),
    PopFrontPendingBackwardsOp,
    PushBackPendingUserRequest((Currency, RequestSendFundsOp)),
    PopFrontPendingUserRequest,
    RemovePendingRequestsCurrency(Currency),
    RemovePendingRequests,
    SetStatus(FriendStatus),
    SetRemoteRelays(Vec<RelayAddress<B>>),
    SetName(String),
    SetRate(Rate),
    SetSentLocalRelays(SentLocalRelays<B>),
}

impl<B> FriendState<B>
where
    B: Clone + CanonicalSerialize,
{
    pub fn new(
        local_public_key: &PublicKey,
        remote_public_key: &PublicKey,
        remote_relays: Vec<RelayAddress<B>>,
        name: String,
    ) -> Self {
        let channel_consistent = ChannelConsistent {
            token_channel: TokenChannel::new(local_public_key, remote_public_key),
            pending_requests: ImVec::new(),
            pending_backwards_ops: ImVec::new(),
            pending_user_requests: ImVec::new(),
            // The remote_max_debt we want to have. When possible, this will be sent to the remote
            // side.
            wanted_remote_max_debt: ImHashMap::new(),
            wanted_local_requests_status: ImHashMap::new(),
            wanted_active_currencies: None,
        };

        FriendState {
            local_public_key: local_public_key.clone(),
            remote_public_key: remote_public_key.clone(),
            remote_relays,
            sent_local_relays: SentLocalRelays::NeverSent,
            name,
            // Initial rate is 0 for a new friend:
            rate: Rate::new(),
            status: FriendStatus::Disabled,
            channel_status: ChannelStatus::Consistent(channel_consistent),
        }
    }

    /*
    // TODO: Do we use this function somewhere?
    /// Find the shared credits we have with this friend.
    /// This value is used for freeze guard calculations.
    /// This value is the capacity shared between the rest of the friends.
    ///
    /// ```text
    ///         ---B
    ///        /
    /// A--*--O-----C
    ///        \
    ///         ---D
    /// ```
    /// In the picture above, the shared credits between O and A will be shared between the nodes
    /// B, C and D.
    ///
    pub fn get_shared_credits(&self) -> u128 {
        let balance = match &self.channel_status {
            ChannelStatus::Consistent(token_channel) => {
                &token_channel.get_mutual_credit().state().balance
            }
            ChannelStatus::Inconsistent(_channel_inconsistent) => return 0,
        };
        balance
            .local_max_debt
            .saturating_add_signed(balance.balance)
    }
    */

    pub fn mutate(&mut self, friend_mutation: &FriendMutation<B>) {
        match friend_mutation {
            FriendMutation::TcMutation(tc_mutation) => match &mut self.channel_status {
                ChannelStatus::Consistent(ref mut channel_consistent) => {
                    channel_consistent.token_channel.mutate(tc_mutation)
                }
                ChannelStatus::Inconsistent(_) => unreachable!(),
            },
            FriendMutation::SetInconsistent(channel_inconsistent) => {
                self.channel_status = ChannelStatus::Inconsistent(channel_inconsistent.clone());
            }
            FriendMutation::SetConsistent(token_channel) => {
                let channel_consistent = ChannelConsistent {
                    token_channel: token_channel.clone(),
                    pending_requests: ImVec::new(),
                    pending_backwards_ops: ImVec::new(),
                    pending_user_requests: ImVec::new(),
                    wanted_remote_max_debt: ImHashMap::new(),
                    wanted_local_requests_status: ImHashMap::new(),
                    wanted_active_currencies: None,
                };
                self.channel_status = ChannelStatus::Consistent(channel_consistent);
            }
            FriendMutation::SetWantedRemoteMaxDebt((currency, wanted_remote_max_debt)) => {
                let channel_consistent = match &mut self.channel_status {
                    ChannelStatus::Consistent(ref mut channel_consistent) => channel_consistent,
                    ChannelStatus::Inconsistent(_) => unreachable!(),
                };

                let _ = channel_consistent
                    .wanted_remote_max_debt
                    .insert(currency.clone(), wanted_remote_max_debt.clone());
            }
            FriendMutation::ClearWantedRemoteMaxDebt(currency) => {
                let channel_consistent = match &mut self.channel_status {
                    ChannelStatus::Consistent(ref mut channel_consistent) => channel_consistent,
                    ChannelStatus::Inconsistent(_) => unreachable!(),
                };

                let res = channel_consistent.wanted_remote_max_debt.remove(currency);
                assert!(res.is_some());
            }
            FriendMutation::SetWantedLocalRequestsStatus((
                currency,
                wanted_local_requests_status,
            )) => {
                let channel_consistent = match &mut self.channel_status {
                    ChannelStatus::Consistent(ref mut channel_consistent) => channel_consistent,
                    ChannelStatus::Inconsistent(_) => unreachable!(),
                };
                let _ = channel_consistent
                    .wanted_local_requests_status
                    .insert(currency.clone(), wanted_local_requests_status.clone());
            }
            FriendMutation::ClearWantedLocalRequestsStatus(currency) => {
                let channel_consistent = match &mut self.channel_status {
                    ChannelStatus::Consistent(ref mut channel_consistent) => channel_consistent,
                    ChannelStatus::Inconsistent(_) => unreachable!(),
                };
                let res = channel_consistent
                    .wanted_local_requests_status
                    .remove(currency);
                assert!(res.is_some());
            }
            FriendMutation::SetWantedActiveCurrencies(currencies) => {
                let channel_consistent = match &mut self.channel_status {
                    ChannelStatus::Consistent(ref mut channel_consistent) => channel_consistent,
                    ChannelStatus::Inconsistent(_) => unreachable!(),
                };
                channel_consistent.wanted_active_currencies = Some(currencies.clone());
            }
            FriendMutation::ClearWantedActiveCurrencies => {
                let channel_consistent = match &mut self.channel_status {
                    ChannelStatus::Consistent(ref mut channel_consistent) => channel_consistent,
                    ChannelStatus::Inconsistent(_) => unreachable!(),
                };
                channel_consistent.wanted_active_currencies = None;
            }
            FriendMutation::PushBackPendingRequest((currency, request_send_funds)) => {
                if let ChannelStatus::Consistent(channel_consistent) = &mut self.channel_status {
                    channel_consistent
                        .pending_requests
                        .push_back((currency.clone(), request_send_funds.clone()));
                } else {
                    unreachable!();
                }
            }
            FriendMutation::PopFrontPendingRequest => {
                if let ChannelStatus::Consistent(channel_consistent) = &mut self.channel_status {
                    channel_consistent.pending_requests.pop_front();
                } else {
                    unreachable!();
                }
            }
            FriendMutation::PushBackPendingBackwardsOp((currency, backwards_op)) => {
                if let ChannelStatus::Consistent(channel_consistent) = &mut self.channel_status {
                    channel_consistent
                        .pending_backwards_ops
                        .push_back((currency.clone(), backwards_op.clone()));
                } else {
                    unreachable!();
                }
            }
            FriendMutation::PopFrontPendingBackwardsOp => {
                if let ChannelStatus::Consistent(channel_consistent) = &mut self.channel_status {
                    channel_consistent.pending_backwards_ops.pop_front();
                } else {
                    unreachable!();
                }
            }
            FriendMutation::PushBackPendingUserRequest((currency, request_send_funds)) => {
                if let ChannelStatus::Consistent(channel_consistent) = &mut self.channel_status {
                    channel_consistent
                        .pending_user_requests
                        .push_back((currency.clone(), request_send_funds.clone()));
                } else {
                    unreachable!();
                }
            }
            FriendMutation::PopFrontPendingUserRequest => {
                if let ChannelStatus::Consistent(channel_consistent) = &mut self.channel_status {
                    channel_consistent.pending_user_requests.pop_front();
                } else {
                    unreachable!();
                }
            }
            FriendMutation::RemovePendingRequestsCurrency(currency) => {
                // Remove all pending outgoing messages for a certain currency.
                if let ChannelStatus::Consistent(channel_consistent) = &mut self.channel_status {
                    channel_consistent
                        .pending_requests
                        .retain(|(currency0, _)| currency0 != currency);

                    channel_consistent
                        .pending_user_requests
                        .retain(|(currency0, _)| currency0 != currency);
                } else {
                    unreachable!();
                }
            }
            FriendMutation::RemovePendingRequests => {
                if let ChannelStatus::Consistent(channel_consistent) = &mut self.channel_status {
                    channel_consistent.pending_requests = ImVec::new();
                    channel_consistent.pending_user_requests = ImVec::new();
                } else {
                    unreachable!();
                }
            }
            FriendMutation::SetStatus(friend_status) => {
                self.status = friend_status.clone();
            }
            FriendMutation::SetRemoteRelays(remote_relays) => {
                self.remote_relays = remote_relays.clone();
            }
            FriendMutation::SetName(friend_name) => {
                self.name = friend_name.clone();
            }
            FriendMutation::SetRate(friend_rate) => {
                self.rate = friend_rate.clone();
            }
            FriendMutation::SetSentLocalRelays(sent_local_relays) => {
                self.sent_local_relays = sent_local_relays.clone();
            }
        }
    }
}
