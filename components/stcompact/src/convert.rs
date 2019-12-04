use app::common::Currency;

use crate::types::{
    BalanceInfo, ChannelConsistentReport, ChannelInconsistentReport, ChannelStatusReport,
    ConfigReport, CountersInfo, FriendLivenessReport, FriendReport, FriendStatusReport, McInfo,
    MoveTokenHashedReport, NodeReport, RequestsStatusReport, ResetTermsReport, TokenInfo,
};

/// A helper util struct, used to implement From for structures that are not from this crate,
/// mostly for ad-hoc tuples used for type conversion.
#[derive(Debug, Clone)]
struct LocalWrapper<T>(pub T);

impl From<app::report::ChannelConsistentReport> for ChannelConsistentReport {
    fn from(_from: app::report::ChannelConsistentReport) -> Self {
        unimplemented!();
    }
}

impl From<app::report::CurrencyBalance> for LocalWrapper<(Currency, i128)> {
    fn from(from: app::report::CurrencyBalance) -> Self {
        LocalWrapper((from.currency, from.balance))
    }
}

impl From<app::report::ResetTermsReport> for ResetTermsReport {
    fn from(from: app::report::ResetTermsReport) -> Self {
        ResetTermsReport {
            reset_token: from.reset_token,
            balance_for_reset: from
                .balance_for_reset
                .into_iter()
                .map(|currency_balance| LocalWrapper::<(Currency, i128)>::from(currency_balance).0)
                .collect(),
        }
    }
}

impl From<app::report::ChannelInconsistentReport> for ChannelInconsistentReport {
    fn from(from: app::report::ChannelInconsistentReport) -> Self {
        ChannelInconsistentReport {
            local_reset_terms: from
                .local_reset_terms
                .into_iter()
                .map(|currency_balance| LocalWrapper::<(Currency, i128)>::from(currency_balance).0)
                .collect(),
            opt_remote_reset_terms: from
                .opt_remote_reset_terms
                .map(|reset_terms_report| reset_terms_report.into()),
        }
    }
}

impl From<app::report::ChannelStatusReport> for ChannelStatusReport {
    fn from(from: app::report::ChannelStatusReport) -> Self {
        match from {
            app::report::ChannelStatusReport::Consistent(channel_consistent_report) => {
                ChannelStatusReport::Consistent(channel_consistent_report.into())
            }
            app::report::ChannelStatusReport::Inconsistent(channel_inconsistent_report) => {
                ChannelStatusReport::Inconsistent(channel_inconsistent_report.into())
            }
        }
    }
}

impl From<app::report::CurrencyBalanceInfo> for LocalWrapper<(Currency, BalanceInfo)> {
    fn from(from: app::report::CurrencyBalanceInfo) -> Self {
        LocalWrapper((
            from.currency,
            BalanceInfo {
                balance: from.balance_info.balance,
                local_pending_debt: from.balance_info.local_pending_debt,
                remote_pending_debt: from.balance_info.remote_pending_debt,
            },
        ))
    }
}

impl From<app::report::McInfo> for McInfo {
    fn from(from: app::report::McInfo) -> Self {
        McInfo {
            local_public_key: from.local_public_key,
            remote_public_key: from.remote_public_key,
            balances: from
                .balances
                .into_iter()
                .map(|currency_balance_info| {
                    LocalWrapper::<(Currency, BalanceInfo)>::from(currency_balance_info).0
                })
                .collect(),
        }
    }
}

impl From<app::report::CountersInfo> for CountersInfo {
    fn from(from: app::report::CountersInfo) -> Self {
        CountersInfo {
            inconsistency_counter: from.inconsistency_counter,
            move_token_counter: from.move_token_counter,
        }
    }
}

impl From<app::report::TokenInfo> for TokenInfo {
    fn from(from: app::report::TokenInfo) -> Self {
        TokenInfo {
            mc: from.mc.into(),
            counters: from.counters.into(),
        }
    }
}

impl From<app::report::MoveTokenHashedReport> for MoveTokenHashedReport {
    fn from(from: app::report::MoveTokenHashedReport) -> Self {
        MoveTokenHashedReport {
            prefix_hash: from.prefix_hash,
            token_info: from.token_info.into(),
            rand_nonce: from.rand_nonce,
            new_token: from.new_token,
        }
    }
}

impl From<app::report::FriendStatusReport> for FriendStatusReport {
    fn from(friend_status_report: app::report::FriendStatusReport) -> Self {
        match friend_status_report {
            app::report::FriendStatusReport::Enabled => FriendStatusReport::Enabled,
            app::report::FriendStatusReport::Disabled => FriendStatusReport::Disabled,
        }
    }
}

impl From<app::report::FriendLivenessReport> for FriendLivenessReport {
    fn from(friend_liveness_report: app::report::FriendLivenessReport) -> Self {
        match friend_liveness_report {
            app::report::FriendLivenessReport::Online => FriendLivenessReport::Online,
            app::report::FriendLivenessReport::Offline => FriendLivenessReport::Offline,
        }
    }
}

impl From<app::report::RequestsStatusReport> for RequestsStatusReport {
    fn from(requests_status_report: app::report::RequestsStatusReport) -> Self {
        match requests_status_report {
            app::report::RequestsStatusReport::Closed => RequestsStatusReport::Closed,
            app::report::RequestsStatusReport::Open => RequestsStatusReport::Open,
        }
    }
}

impl From<app::report::CurrencyConfigReport> for LocalWrapper<(Currency, ConfigReport)> {
    fn from(currency_config_report: app::report::CurrencyConfigReport) -> Self {
        LocalWrapper((
            currency_config_report.currency,
            ConfigReport {
                rate: currency_config_report.rate,
                wanted_remote_max_debt: currency_config_report.wanted_remote_max_debt,
                wanted_local_requests_status: currency_config_report
                    .wanted_local_requests_status
                    .into(),
            },
        ))
    }
}

impl From<app::report::FriendReport> for FriendReport {
    fn from(friend_report: app::report::FriendReport) -> Self {
        FriendReport {
            name: friend_report.name,
            currency_configs: friend_report
                .currency_configs
                .into_iter()
                .map(|currency_config_report| {
                    LocalWrapper::<(Currency, ConfigReport)>::from(currency_config_report).0
                })
                .collect(),
            opt_last_incoming_move_token: friend_report
                .opt_last_incoming_move_token
                .map(|last_incoming_move_token| last_incoming_move_token.into()),
            liveness: friend_report.liveness.into(),
            channel_status: friend_report.channel_status.into(),
            status: friend_report.status.into(),
        }
    }
}

impl From<app::report::NodeReport> for NodeReport {
    fn from(node_report: app::report::NodeReport) -> Self {
        NodeReport {
            local_public_key: node_report.funder_report.local_public_key,
            index_servers: node_report.index_client_report.index_servers,
            opt_connected_index_server: node_report.index_client_report.opt_connected_server,
            relays: node_report.funder_report.relays,
            friends: node_report
                .funder_report
                .friends
                .into_iter()
                .map(|(friend_public_key, friend_report)| (friend_public_key, friend_report.into()))
                .collect(),
        }
    }
}
