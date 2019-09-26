use byteorder::{BigEndian, WriteBytesExt};

use crypto::hash::{self, sha_512_256};

use proto::crypto::HashResult;

use common::int_convert::usize_to_u64;

use crate::canonical::CanonicalSerialize;
use proto::funder::messages::{MoveToken, PendingTransaction, ResponseSendFundsOp, TokenInfo};
use proto::index_server::messages::MutationsUpdate;
use proto::report::messages::MoveTokenHashedReport;

pub const FUNDS_RESPONSE_PREFIX: &[u8] = b"FUND_RESPONSE";
pub const FUNDS_CANCEL_PREFIX: &[u8] = b"FUND_CANCEL";

/// Create the buffer we sign over at the Response funds.
/// Note that the signature is not just over the Response funds bytes. The signed buffer also
/// contains information from the Request funds.
pub fn create_response_signature_buffer<S>(
    response_send_funds: &ResponseSendFundsOp<S>,
    pending_transaction: &PendingTransaction,
) -> Vec<u8> {
    let mut sbuffer = Vec::new();

    sbuffer.extend_from_slice(&hash::sha_512_256(FUNDS_RESPONSE_PREFIX));

    let mut inner_blob = Vec::new();
    inner_blob.extend_from_slice(&pending_transaction.request_id);
    inner_blob.extend_from_slice(&response_send_funds.rand_nonce);

    sbuffer.extend_from_slice(&hash::sha_512_256(&inner_blob));
    sbuffer.extend_from_slice(&pending_transaction.src_hashed_lock);
    sbuffer.extend_from_slice(&response_send_funds.dest_hashed_lock);
    sbuffer
        .write_u128::<BigEndian>(pending_transaction.dest_payment)
        .unwrap();
    sbuffer
        .write_u128::<BigEndian>(pending_transaction.total_dest_payment)
        .unwrap();
    sbuffer.extend_from_slice(&pending_transaction.invoice_id);

    sbuffer
}

// Prefix used for chain hashing of token channel funds.
// NEXT is used for hashing for the next move token funds.
pub const TOKEN_NEXT: &[u8] = b"NEXT";

/// Combine all operations into one hash value.
pub fn operations_hash<B>(move_token: &MoveToken<B>) -> HashResult {
    let mut operations_data = Vec::new();
    operations_data
        .write_u64::<BigEndian>(usize_to_u64(move_token.operations.len()).unwrap())
        .unwrap();
    for op in &move_token.operations {
        operations_data.extend_from_slice(&op.canonical_serialize());
    }
    sha_512_256(&operations_data)
}

/// Combine all operations into one hash value.
pub fn local_address_hash<B>(move_token: &MoveToken<B>) -> HashResult
where
    B: CanonicalSerialize,
{
    sha_512_256(&move_token.opt_local_relays.canonical_serialize())
}

pub fn hash_token_info(token_info: &TokenInfo) -> HashResult {
    let mut hash_buff = Vec::new();
    hash_buff.extend_from_slice(&token_info.local_public_key);
    hash_buff.extend_from_slice(&token_info.remote_public_key);
    hash_buff
        .write_u64::<BigEndian>(token_info.inconsistency_counter)
        .unwrap();
    hash_buff
        .write_u128::<BigEndian>(token_info.move_token_counter)
        .unwrap();
    hash_buff
        .write_i128::<BigEndian>(token_info.balance)
        .unwrap();
    hash_buff
        .write_u128::<BigEndian>(token_info.local_pending_debt)
        .unwrap();
    hash_buff
        .write_u128::<BigEndian>(token_info.remote_pending_debt)
        .unwrap();
    sha_512_256(&hash_buff)
}

/// Hash operations and local_address:
pub fn prefix_hash<B, S>(move_token: &MoveToken<B, S>) -> HashResult
where
    B: CanonicalSerialize,
{
    let mut hash_buff = Vec::new();

    hash_buff.extend_from_slice(&move_token.old_token);

    // TODO: Use CanonicalSerialize instead here:
    hash_buff
        .write_u64::<BigEndian>(usize_to_u64(move_token.operations.len()).unwrap())
        .unwrap();
    for op in &move_token.operations {
        hash_buff.extend_from_slice(&op.canonical_serialize());
    }

    hash_buff.extend_from_slice(&move_token.opt_local_relays.canonical_serialize());

    sha_512_256(&hash_buff)
}

pub fn move_token_signature_buff<B, S>(move_token: &MoveToken<B, S>) -> Vec<u8>
where
    B: CanonicalSerialize,
{
    let mut sig_buffer = Vec::new();
    sig_buffer.extend_from_slice(&sha_512_256(TOKEN_NEXT));
    sig_buffer.extend_from_slice(&prefix_hash(move_token));
    sig_buffer.extend_from_slice(&move_token.info_hash);
    sig_buffer.extend_from_slice(&move_token.rand_nonce);
    sig_buffer
}

pub const MUTATIONS_UPDATE_PREFIX: &[u8] = b"MUTATIONS_UPDATE";

pub fn create_mutations_update_signature_buff(mutations_update: &MutationsUpdate) -> Vec<u8> {
    let mut res_bytes = Vec::new();
    res_bytes.extend_from_slice(&hash::sha_512_256(MUTATIONS_UPDATE_PREFIX));
    res_bytes.extend_from_slice(&mutations_update.node_public_key);

    res_bytes
        .write_u64::<BigEndian>(usize_to_u64(mutations_update.index_mutations.len()).unwrap())
        .unwrap();
    for mutation in &mutations_update.index_mutations {
        res_bytes.extend(mutation.canonical_serialize());
    }

    res_bytes.extend_from_slice(&mutations_update.time_hash);
    res_bytes.extend_from_slice(&mutations_update.session_id);
    res_bytes
        .write_u64::<BigEndian>(mutations_update.counter)
        .unwrap();
    res_bytes.extend_from_slice(&mutations_update.rand_nonce);

    res_bytes
}

pub fn move_token_hashed_report_signature_buff(
    move_token_hashed_report: &MoveTokenHashedReport,
) -> Vec<u8> {
    let token_info = TokenInfo {
        local_public_key: move_token_hashed_report.local_public_key.clone(),
        remote_public_key: move_token_hashed_report.remote_public_key.clone(),
        inconsistency_counter: move_token_hashed_report.inconsistency_counter,
        move_token_counter: move_token_hashed_report.move_token_counter,
        balance: move_token_hashed_report.balance,
        local_pending_debt: move_token_hashed_report.local_pending_debt,
        remote_pending_debt: move_token_hashed_report.remote_pending_debt,
    };

    let mut sig_buffer = Vec::new();
    sig_buffer.extend_from_slice(&sha_512_256(TOKEN_NEXT));
    sig_buffer.extend_from_slice(&move_token_hashed_report.prefix_hash);
    sig_buffer.extend_from_slice(&hash_token_info(&token_info));
    sig_buffer.extend_from_slice(&move_token_hashed_report.rand_nonce);
    sig_buffer
}
