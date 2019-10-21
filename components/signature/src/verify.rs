use byteorder::{BigEndian, WriteBytesExt};

use crypto::hash;
use crypto::hash_lock::HashLock;
use crypto::identity::verify_signature;

use proto::crypto::{InvoiceId, PublicKey};

use proto::funder::messages::{Commit, Currency, MoveToken, MultiCommit, Receipt};
use proto::index_server::messages::MutationsUpdate;
use proto::report::messages::MoveTokenHashedReport;

use crate::canonical::CanonicalSerialize;
use crate::signature_buff::{
    create_mutations_update_signature_buff, move_token_hashed_report_signature_buff,
    move_token_signature_buff, FUNDS_RESPONSE_PREFIX,
};

/// Verify that a given receipt's signature is valid
pub fn verify_receipt(receipt: &Receipt, public_key: &PublicKey) -> bool {
    let mut data = Vec::new();

    data.extend_from_slice(&hash::sha_512_256(FUNDS_RESPONSE_PREFIX));
    data.extend(receipt.response_hash.as_ref());
    data.extend_from_slice(&receipt.src_plain_lock.hash_lock());
    data.extend_from_slice(&receipt.dest_plain_lock.hash_lock());
    data.write_u128::<BigEndian>(receipt.dest_payment).unwrap();
    data.write_u128::<BigEndian>(receipt.total_dest_payment)
        .unwrap();
    data.extend(receipt.invoice_id.as_ref());
    data.extend_from_slice(&receipt.currency.canonical_serialize());
    verify_signature(&data, public_key, &receipt.signature)
}

/// Verify that a given Commit signature is valid
fn verify_commit(
    commit: &Commit,
    invoice_id: &InvoiceId,
    currency: &Currency,
    total_dest_payment: u128,
    local_public_key: &PublicKey,
) -> bool {
    let mut data = Vec::new();

    data.extend_from_slice(&hash::sha_512_256(FUNDS_RESPONSE_PREFIX));
    data.extend(commit.response_hash.as_ref());
    data.extend_from_slice(&commit.src_plain_lock.hash_lock());
    data.extend_from_slice(&commit.dest_hashed_lock);
    data.write_u128::<BigEndian>(commit.dest_payment).unwrap();
    data.write_u128::<BigEndian>(total_dest_payment).unwrap();
    data.extend(invoice_id.as_ref());
    data.extend_from_slice(&currency.canonical_serialize());
    verify_signature(&data, local_public_key, &commit.signature)
}

// TODO: Possibly split nicely into two functions?
/// Verify that all the Commit-s inside a MultiCommit are valid
pub fn verify_multi_commit(multi_commit: &MultiCommit, local_public_key: &PublicKey) -> bool {
    let mut is_sig_valid = true;
    for commit in &multi_commit.commits {
        // We don't exit immediately on verification failure to get a constant time verification.
        // (Not sure if this is really important here)
        is_sig_valid &= verify_commit(
            commit,
            &multi_commit.invoice_id,
            &multi_commit.currency,
            multi_commit.total_dest_payment,
            local_public_key,
        );
    }
    if !is_sig_valid {
        return false;
    }

    // Check if the credits add up:
    let mut sum_credits = 0u128;
    for commit in &multi_commit.commits {
        sum_credits = if let Some(sum_credits) = sum_credits.checked_add(commit.dest_payment) {
            sum_credits
        } else {
            return false;
        }
    }

    // Require that the multi_commit.total_dest_payment matches the sum of all commit.dest_payment:
    sum_credits == multi_commit.total_dest_payment
}

/// Verify that new_token is a valid signature over the rest of the fields.
pub fn verify_move_token<B>(move_token: &MoveToken<B>, public_key: &PublicKey) -> bool
where
    B: CanonicalSerialize,
{
    let sig_buffer = move_token_signature_buff(move_token);
    verify_signature(&sig_buffer, public_key, &move_token.new_token)
}

/// Verify the signature at the MutationsUpdate structure.
/// Note that this structure also contains the `node_public_key` field, which is the identity
/// of the node who signed this struct.
pub fn verify_mutations_update(mutations_update: &MutationsUpdate) -> bool {
    let signature_buff = create_mutations_update_signature_buff(&mutations_update);
    verify_signature(
        &signature_buff,
        &mutations_update.node_public_key,
        &mutations_update.signature,
    )
}

// TODO: Is the public_key argument redundant now? (As it should be exactly the same
// as move_token_hashed_report.local_public_key)
/// Verify that new_token is a valid signature over the rest of the fields.
pub fn verify_move_token_hashed_report(
    move_token_hashed_report: &MoveTokenHashedReport,
    public_key: &PublicKey,
) -> bool {
    let sig_buffer = move_token_hashed_report_signature_buff(move_token_hashed_report);
    verify_signature(&sig_buffer, public_key, &move_token_hashed_report.new_token)
}
