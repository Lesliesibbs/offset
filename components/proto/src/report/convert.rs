#![allow(unused)]
use std::collections::HashMap;
use std::fmt::Debug;

use common::mutable_state::MutableState;
use common::safe_arithmetic::{SafeSignedArithmetic, SafeUnsignedArithmetic};

use crate::crypto::PublicKey;

use crate::index_client::messages::{FriendInfo, IndexClientState};
use crate::index_server::messages::{IndexMutation, UpdateFriend};

use crate::report::messages::{
    ChannelStatusReport, FriendLivenessReport, FriendReport, FriendStatusReport, FunderReport,
    FunderReportMutation, RequestsStatusReport,
};

// Conversion to index client mutations and state
// ----------------------------------------------

// This code is used as glue between FunderReport structure and input mutations given to
// `index_client`. This allows the offst-index-client crate to not depend on the offst-funder
// crate.

// TODO: Maybe this logic shouldn't be here? Where should we move it to?
// TODO: Add tests (Mostly for arithmetic stuff here)

/*
 * TODO: Restore this code later (When adjusting index client):

/// Calculate send and receive capacities for a given `friend_report`.
fn calc_friend_capacities<B>(friend_report: &FriendReport<B>) -> (u128, u128)
where
    B: Clone,
{
    if friend_report.status == FriendStatusReport::Disabled
        || friend_report.liveness == FriendLivenessReport::Offline
    {
        return (0, 0);
    }

    let tc_report = match &friend_report.channel_status {
        ChannelStatusReport::Inconsistent(_) => return (0, 0),
        ChannelStatusReport::Consistent(channel_consistent_report) => {
            &channel_consistent_report.tc_report
        }
    };

    let balance = &tc_report.balance;

    let send_capacity = if tc_report.requests_status.remote == RequestsStatusReport::Closed {
        0
    } else {
        // local_max_debt + balance - local_pending_debt
        balance.local_max_debt.saturating_add_signed(
            balance
                .balance
                .checked_sub_unsigned(balance.local_pending_debt)
                .unwrap(),
        )
    };

    let recv_capacity = if tc_report.requests_status.local == RequestsStatusReport::Closed {
        0
    } else {
        balance.remote_max_debt.saturating_sub_signed(
            balance
                .balance
                .checked_add_unsigned(balance.remote_pending_debt)
                .unwrap(),
        )
    };

    (send_capacity, recv_capacity)
}

pub fn funder_report_to_index_client_state<B>(funder_report: &FunderReport<B>) -> IndexClientState
where
    B: Clone,
{
    let friends = funder_report
        .friends
        .iter()
        .map(|(friend_public_key, friend_report)| {
            let (send_capacity, recv_capacity) = calc_friend_capacities(friend_report);
            (
                friend_public_key.clone(),
                FriendInfo {
                    send_capacity,
                    recv_capacity,
                    rate: friend_report.rate.clone(),
                },
            )
        })
        .filter(|(_, friend_info)| friend_info.send_capacity != 0 || friend_info.recv_capacity != 0)
        .collect::<HashMap<PublicKey, FriendInfo>>();

    IndexClientState { friends }
}

pub fn funder_report_mutation_to_index_mutation<B>(
    funder_report: &FunderReport<B>,
    funder_report_mutation: &FunderReportMutation<B>,
) -> Option<IndexMutation>
where
    B: Clone + Debug,
{
    let create_update_friend = |public_key: &PublicKey| {
        let opt_old_capacities_rate =
            funder_report
                .friends
                .get(public_key)
                .map(|old_friend_report| {
                    (
                        calc_friend_capacities(&old_friend_report),
                        old_friend_report.rate.clone(),
                    )
                });

        let mut new_funder_report = funder_report.clone();
        new_funder_report.mutate(funder_report_mutation).unwrap();

        let new_friend_report = new_funder_report.friends.get(public_key).unwrap(); // We assert that a new friend was added

        let new_capacities = calc_friend_capacities(new_friend_report);
        let new_rate = new_friend_report.rate.clone();

        // Return UpdateFriend if something has changed:
        if opt_old_capacities_rate != Some((new_capacities, new_rate.clone())) {
            let (send_capacity, recv_capacity) = new_capacities;
            let update_friend = UpdateFriend {
                public_key: public_key.clone(),
                send_capacity,
                recv_capacity,
                rate: new_rate,
            };
            Some(IndexMutation::UpdateFriend(update_friend))
        } else {
            None
        }
    };

    match funder_report_mutation {
        FunderReportMutation::AddRelay(_)
        | FunderReportMutation::RemoveRelay(_)
        | FunderReportMutation::SetNumOpenInvoices(_)
        | FunderReportMutation::SetNumPayments(_)
        | FunderReportMutation::SetNumOpenTransactions(_) => None,
        FunderReportMutation::AddFriend(add_friend_report) => {
            create_update_friend(&add_friend_report.friend_public_key)
        }
        FunderReportMutation::RemoveFriend(public_key) => {
            Some(IndexMutation::RemoveFriend(public_key.clone()))
        }
        FunderReportMutation::PkFriendReportMutation((public_key, _friend_report_mutation)) => {
            create_update_friend(&public_key)
        }
    }
}
*/

// TODO: Add tests.
