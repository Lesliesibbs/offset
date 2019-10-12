use std::fmt::Debug;

use signature::canonical::CanonicalSerialize;

use crypto::rand::{CryptoRandom, RandGen};

use proto::crypto::{PublicKey, Uid};
use proto::funder::messages::{Currency, FunderOutgoingControl, RequestResult, TransactionResult};

use crate::handler::state_wrap::MutableFunderState;
use crate::handler::types::SendCommands;
use crate::handler::utils::find_request_origin;

use crate::friend::{BackwardsOp, ChannelStatus, FriendMutation};
use crate::state::{FunderMutation, Payment};
use crate::types::{create_cancel_send_funds, create_pending_transaction};

#[derive(Debug)]
pub enum CurrencyChoice {
    /// Just one currency
    One(Currency),
    /// All currency
    All,
}

/// Reply to a single request message with a cancellation.
pub fn reply_with_cancel<B>(
    m_state: &mut MutableFunderState<B>,
    send_commands: &mut SendCommands,
    remote_public_key: &PublicKey,
    currency: &Currency,
    request_id: &Uid,
) where
    B: Clone + CanonicalSerialize + PartialEq + Eq + Debug,
{
    let cancel_send_funds = create_cancel_send_funds(request_id.clone());
    let friend_mutation = FriendMutation::PushBackPendingBackwardsOp((
        currency.clone(),
        BackwardsOp::Cancel(cancel_send_funds),
    ));
    let funder_mutation =
        FunderMutation::FriendMutation((remote_public_key.clone(), friend_mutation));
    m_state.mutate(funder_mutation);
    send_commands.set_try_send(remote_public_key);
}

/// Remove a local transaction (Where this node is the buyer side)
pub fn remove_transaction<B, R>(m_state: &mut MutableFunderState<B>, rng: &R, request_id: &Uid)
where
    B: Clone + CanonicalSerialize + PartialEq + Eq + Debug,
    R: CryptoRandom,
{
    // TODO: Can this unwrap() ever panic?
    let open_transaction = m_state
        .state()
        .open_transactions
        .get(request_id)
        .unwrap()
        .clone();

    // TODO: Can this unwrap() ever panic?
    let payment = m_state
        .state()
        .payments
        .get(&open_transaction.payment_id)
        .unwrap()
        .clone();

    // Remove transaction:
    let funder_mutation = FunderMutation::RemoveTransaction(request_id.clone());
    m_state.mutate(funder_mutation);

    // Update payment:
    // - Decrease num_transactions
    // - Possibly remove payment
    let opt_new_payment = match payment {
        Payment::NewTransactions(new_transactions) => {
            let mut new_new_transactions = new_transactions.clone();
            new_new_transactions.num_transactions =
                new_transactions.num_transactions.checked_sub(1).unwrap();
            Some(Payment::NewTransactions(new_new_transactions))
        }
        Payment::InProgress(num_transactions) => {
            let new_num_transactions = num_transactions.checked_sub(1).unwrap();
            if new_num_transactions > 0 {
                Some(Payment::InProgress(new_num_transactions))
            } else {
                let ack_uid = Uid::rand_gen(rng);
                Some(Payment::Canceled(ack_uid))
            }
        }
        Payment::Success((num_transactions, receipt, request_id)) => {
            let new_num_transactions = num_transactions.checked_sub(1).unwrap();
            Some(Payment::Success((
                new_num_transactions,
                receipt.clone(),
                request_id,
            )))
        }
        Payment::Canceled(_) => {
            unreachable!();
        }
        Payment::AfterSuccessAck(num_transactions) => {
            let new_num_transactions = num_transactions.checked_sub(1).unwrap();
            if new_num_transactions > 0 {
                Some(Payment::AfterSuccessAck(new_num_transactions))
            } else {
                None
            }
        }
    };

    let funder_mutation = if let Some(new_payment) = opt_new_payment {
        FunderMutation::UpdatePayment((open_transaction.payment_id, new_payment))
    } else {
        FunderMutation::RemovePayment(open_transaction.payment_id)
    };
    m_state.mutate(funder_mutation);
}

/// Cancel outgoing local requests that are already inside the token channel (Possibly already
/// communicated to the remote side). This is a violent operation, as we break our promises for
/// forwarded requests. This should only be done during unfriending.
pub fn cancel_local_pending_transactions<B, R>(
    m_state: &mut MutableFunderState<B>,
    send_commands: &mut SendCommands,
    outgoing_control: &mut Vec<FunderOutgoingControl<B>>,
    rng: &R,
    friend_public_key: &PublicKey,
) where
    B: Clone + CanonicalSerialize + PartialEq + Eq + Debug,
    R: CryptoRandom,
{
    let friend = m_state.state().friends.get(friend_public_key).unwrap();

    let token_channel = match &friend.channel_status {
        ChannelStatus::Inconsistent(_) => unreachable!(),
        ChannelStatus::Consistent(channel_consistent) => &channel_consistent.token_channel,
    };

    for (currency, mutual_credit) in token_channel.get_mutual_credits().clone() {
        // Mark all pending requests to this friend as errors.
        // As the token channel is being reset, we can be sure we will never obtain a response
        // for those requests.
        let pending_local_transactions = mutual_credit.state().pending_transactions.local.clone();

        // Prepare a list of all remote requests that we need to cancel:
        for (local_request_id, pending_local_transaction) in pending_local_transactions {
            let opt_origin_public_key =
                find_request_origin(m_state.state(), &currency, &local_request_id).cloned();
            match opt_origin_public_key {
                Some(origin_public_key) => {
                    // We have found the friend that is the origin of this request.
                    // We send him a cancel message.
                    let cancel_send_funds =
                        create_cancel_send_funds(pending_local_transaction.request_id);
                    let friend_mutation = FriendMutation::PushBackPendingBackwardsOp((
                        currency.clone(),
                        BackwardsOp::Cancel(cancel_send_funds),
                    ));
                    let funder_mutation = FunderMutation::FriendMutation((
                        origin_public_key.clone(),
                        friend_mutation,
                    ));
                    m_state.mutate(funder_mutation);
                    // TODO: Should we add currency as argument to set_try_send()?
                    send_commands.set_try_send(&origin_public_key);
                }
                None => {
                    // We are the origin of this request.
                    // We send a cancel message through the control:
                    let transaction_result = TransactionResult {
                        request_id: pending_local_transaction.request_id.clone(),
                        result: RequestResult::Failure,
                    };
                    outgoing_control
                        .push(FunderOutgoingControl::TransactionResult(transaction_result));
                    remove_transaction(m_state, rng, &pending_local_transaction.request_id);
                }
            };
        }
    }
}

/// Cancel all pending request messages at the pending_requests queue.
/// These are requests that were forwarded from other nodes.
pub fn cancel_pending_requests<B, R>(
    m_state: &mut MutableFunderState<B>,
    send_commands: &mut SendCommands,
    outgoing_control: &mut Vec<FunderOutgoingControl<B>>,
    rng: &R,
    friend_public_key: &PublicKey,
    currency_choice: &CurrencyChoice,
) where
    B: Clone + CanonicalSerialize + PartialEq + Eq + Debug,
    R: CryptoRandom,
{
    let friend = m_state.state().friends.get(friend_public_key).unwrap();
    let channel_consistent = match &friend.channel_status {
        ChannelStatus::Inconsistent(_) => unreachable!(),
        ChannelStatus::Consistent(channel_consistent) => channel_consistent,
    };
    let mut currency_queues = channel_consistent.currency_queues.clone();

    let currencies = if let CurrencyChoice::One(currency) = currency_choice {
        vec![currency.clone()]
    } else {
        currency_queues.keys().cloned().collect()
    };

    for currency in currencies {
        let channel_queues = currency_queues.get_mut(&currency).unwrap();
        let mut pending_requests = &mut channel_queues.pending_requests;
        while let Some(pending_request) = pending_requests.pop_front() {
            let friend_mutation = FriendMutation::PopFrontPendingRequest(currency.clone());
            let funder_mutation =
                FunderMutation::FriendMutation((friend_public_key.clone(), friend_mutation));
            m_state.mutate(funder_mutation);

            let opt_origin_public_key =
                find_request_origin(m_state.state(), &currency, &pending_request.request_id)
                    .cloned();
            match opt_origin_public_key {
                Some(origin_public_key) => {
                    let pending_local_transaction = create_pending_transaction(&pending_request);
                    let cancel_send_funds =
                        create_cancel_send_funds(pending_local_transaction.request_id);
                    let friend_mutation = FriendMutation::PushBackPendingBackwardsOp((
                        currency.clone(),
                        BackwardsOp::Cancel(cancel_send_funds),
                    ));
                    let funder_mutation = FunderMutation::FriendMutation((
                        origin_public_key.clone(),
                        friend_mutation,
                    ));
                    m_state.mutate(funder_mutation);
                    send_commands.set_try_send(&origin_public_key);
                }
                None => {
                    // We are the origin of this request:
                    let transaction_result = TransactionResult {
                        request_id: pending_request.request_id.clone(),
                        result: RequestResult::Failure,
                    };
                    outgoing_control
                        .push(FunderOutgoingControl::TransactionResult(transaction_result));
                    remove_transaction(m_state, rng, &pending_request.request_id);
                }
            };
        }
    }
}

/// Cancel all pending request messages at the user_pending_requests queue.
/// These are requests that were created by the local user of this node.
pub fn cancel_pending_user_requests<B, R>(
    m_state: &mut MutableFunderState<B>,
    outgoing_control: &mut Vec<FunderOutgoingControl<B>>,
    rng: &R,
    friend_public_key: &PublicKey,
    currency_choice: &CurrencyChoice,
) where
    B: Clone + CanonicalSerialize + PartialEq + Eq + Debug,
    R: CryptoRandom,
{
    let friend = m_state.state().friends.get(&friend_public_key).unwrap();
    let channel_consistent = match &friend.channel_status {
        ChannelStatus::Inconsistent(_) => unreachable!(),
        ChannelStatus::Consistent(channel_consistent) => channel_consistent,
    };
    let mut currency_queues = channel_consistent.currency_queues.clone();

    let currencies = if let CurrencyChoice::One(currency) = currency_choice {
        vec![currency.clone()]
    } else {
        currency_queues.keys().cloned().collect()
    };

    for currency in currencies {
        let channel_queues = currency_queues.get_mut(&currency).unwrap();
        let mut pending_user_requests = &mut channel_queues.pending_user_requests;
        while let Some(pending_user_request) = pending_user_requests.pop_front() {
            let friend_mutation = FriendMutation::PopFrontPendingUserRequest(currency.clone());
            let funder_mutation =
                FunderMutation::FriendMutation((friend_public_key.clone(), friend_mutation));
            m_state.mutate(funder_mutation);

            // We are the origin of this request:
            let transaction_result = TransactionResult {
                request_id: pending_user_request.request_id.clone(),
                result: RequestResult::Failure,
            };
            outgoing_control.push(FunderOutgoingControl::TransactionResult(transaction_result));
            remove_transaction(m_state, rng, &pending_user_request.request_id);
        }
    }
}
