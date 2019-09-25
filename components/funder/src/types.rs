use signature::canonical::CanonicalSerialize;

use proto::crypto::{HashResult, HashedLock, PublicKey, RandValue, Signature, Uid};

use proto::app_server::messages::RelayAddress;
use proto::funder::messages::{
    CancelSendFundsOp, ChannelerUpdateFriend, CollectSendFundsOp, FriendMessage, FriendTcOp,
    FunderIncomingControl, FunderOutgoingControl, MoveToken, TokenInfo, PendingTransaction,
    RequestSendFundsOp, ResponseSendFundsOp, TransactionStage,
};

use signature::signature_buff::{
    create_response_signature_buffer, move_token_signature_buff, prefix_hash, hash_token_info,
};

use identity::IdentityClient;

pub type UnsignedResponseSendFundsOp = ResponseSendFundsOp<()>;
pub type UnsignedMoveToken<B> = MoveToken<B, ()>;

pub async fn sign_move_token<'a, B>(
    unsigned_move_token: UnsignedMoveToken<B>,
    token_info: &TokenInfo, 
    identity_client: &'a mut IdentityClient,
) -> MoveToken<B>
where
    B: CanonicalSerialize + 'a,
{
    let signature_buff = move_token_signature_buff(&unsigned_move_token);
    let new_token = identity_client
        .request_signature(signature_buff)
        .await
        .unwrap();

    MoveToken {
        operations: unsigned_move_token.operations,
        opt_local_relays: unsigned_move_token.opt_local_relays,
        old_token: unsigned_move_token.old_token,
        info_hash: hash_token_info(token_info),
        /*
        local_public_key: unsigned_move_token.local_public_key,
        remote_public_key: unsigned_move_token.remote_public_key,
        inconsistency_counter: unsigned_move_token.inconsistency_counter,
        move_token_counter: unsigned_move_token.move_token_counter,
        balance: unsigned_move_token.balance,
        local_pending_debt: unsigned_move_token.local_pending_debt,
        remote_pending_debt: unsigned_move_token.remote_pending_debt,
        */
        rand_nonce: unsigned_move_token.rand_nonce,
        new_token,
    }
}

pub async fn create_response_send_funds<'a>(
    pending_transaction: &'a PendingTransaction,
    dest_hashed_lock: HashedLock,
    rand_nonce: RandValue,
    identity_client: &'a mut IdentityClient,
) -> ResponseSendFundsOp {
    let u_response_send_funds = ResponseSendFundsOp {
        request_id: pending_transaction.request_id.clone(),
        dest_hashed_lock,
        rand_nonce,
        signature: (),
    };

    let signature_buff =
        create_response_signature_buffer(&u_response_send_funds, pending_transaction);
    let signature = identity_client
        .request_signature(signature_buff)
        .await
        .unwrap();

    ResponseSendFundsOp {
        request_id: u_response_send_funds.request_id,
        dest_hashed_lock: u_response_send_funds.dest_hashed_lock,
        rand_nonce: u_response_send_funds.rand_nonce,
        signature,
    }
}

pub fn create_cancel_send_funds(request_id: Uid) -> CancelSendFundsOp {
    CancelSendFundsOp { request_id }
}

pub fn create_pending_transaction(request_send_funds: &RequestSendFundsOp) -> PendingTransaction {
    PendingTransaction {
        request_id: request_send_funds.request_id.clone(),
        route: request_send_funds.route.clone(),
        dest_payment: request_send_funds.dest_payment,
        total_dest_payment: request_send_funds.total_dest_payment,
        invoice_id: request_send_funds.invoice_id.clone(),
        left_fees: request_send_funds.left_fees,
        src_hashed_lock: request_send_funds.src_hashed_lock.clone(),
        stage: TransactionStage::Request,
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum UnsignedFriendTcOp {
    EnableRequests,
    DisableRequests,
    SetRemoteMaxDebt(u128),
    RequestSendFunds(RequestSendFundsOp),
    ResponseSendFunds(ResponseSendFundsOp),
    UnsignedResponseSendFunds(UnsignedResponseSendFundsOp),
    CancelSendFunds(CancelSendFundsOp),
    CollectSendFunds(CollectSendFundsOp),
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct MoveTokenHashed {
    /// Hash of operations and local_relays
    pub prefix_hash: HashResult,
    pub token_info: TokenInfo,
    /*
    pub local_public_key: PublicKey,
    pub remote_public_key: PublicKey,
    pub inconsistency_counter: u64,
    pub move_token_counter: u128,
    pub balance: i128,
    pub local_pending_debt: u128,
    pub remote_pending_debt: u128,
    */
    pub rand_nonce: RandValue,
    pub new_token: Signature,
}

pub fn create_unsigned_move_token<B>(
    operations: Vec<FriendTcOp>,
    opt_local_relays: Option<Vec<RelayAddress<B>>>,
    token_info: &TokenInfo,
    old_token: Signature,
    /*
    local_public_key: PublicKey,
    remote_public_key: PublicKey,
    inconsistency_counter: u64,
    move_token_counter: u128,
    balance: i128,
    local_pending_debt: u128,
    remote_pending_debt: u128,
    */
    rand_nonce: RandValue,
) -> UnsignedMoveToken<B> {
    MoveToken {
        operations,
        opt_local_relays,
        info_hash: hash_token_info(token_info),
        old_token,
        /*
        local_public_key,
        remote_public_key,
        inconsistency_counter,
        move_token_counter,
        balance,
        local_pending_debt,
        remote_pending_debt,
        */
        rand_nonce,
        new_token: (),
    }
}

/*
pub async fn create_move_token<A>(operations: Vec<FriendTcOp>,
                 opt_local_relays: Option<A>,
                 old_token: Signature,
                 inconsistency_counter: u64,
                 move_token_counter: u128,
                 balance: i128,
                 local_pending_debt: u128,
                 remote_pending_debt: u128,
                 rand_nonce: RandValue,
                 identity_client: IdentityClient) -> MoveToken<A>
where
    A: CanonicalSerialize,
{

    let mut move_token = MoveToken {
        operations,
        opt_local_relays,
        old_token,
        inconsistency_counter,
        move_token_counter,
        balance,
        local_pending_debt,
        remote_pending_debt,
        rand_nonce,
        new_token: Signature::zero(),
    };

    let sig_buffer = move_token_signature_buff(&move_token);
    move_token.new_token = identity_client.request_signature(sig_buffer).await.unwrap();
    move_token
}
*/

/// Create a hashed version of the MoveToken.
/// Hashed version contains the hash of the operations instead of the operations themselves,
/// hence it is usually shorter.
pub fn create_hashed<B>(move_token: &MoveToken<B>, token_info: &TokenInfo) -> MoveTokenHashed
where
    B: CanonicalSerialize,
{
    MoveTokenHashed {
        prefix_hash: prefix_hash(move_token),
        token_info: token_info.clone(),
        /*
        local_public_key: move_token.local_public_key.clone(),
        remote_public_key: move_token.remote_public_key.clone(),
        inconsistency_counter: move_token.inconsistency_counter,
        move_token_counter: move_token.move_token_counter,
        balance: move_token.balance,
        local_pending_debt: move_token.local_pending_debt,
        remote_pending_debt: move_token.remote_pending_debt,
        */
        rand_nonce: move_token.rand_nonce.clone(),
        new_token: move_token.new_token.clone(),
    }
}

#[derive(Debug, Clone)]
pub enum IncomingLivenessMessage {
    Online(PublicKey),
    Offline(PublicKey),
}

pub struct FriendInconsistencyError {
    pub reset_token: Signature,
    pub balance_for_reset: i128,
}

#[derive(Debug)]
pub enum ChannelerConfig<RA> {
    /// Set relay address for local node
    /// This is the address the Channeler will connect to
    /// and listen for new connections
    SetRelays(Vec<RA>),
    UpdateFriend(ChannelerUpdateFriend<RA>),
    RemoveFriend(PublicKey),
}

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum FunderIncomingComm<B> {
    Liveness(IncomingLivenessMessage),
    Friend((PublicKey, FriendMessage<B>)),
}

/// An incoming message to the Funder:
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum FunderIncoming<B> {
    Init,
    Control(FunderIncomingControl<B>),
    Comm(FunderIncomingComm<B>),
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum FunderOutgoing<B>
where
    B: Clone,
{
    Control(FunderOutgoingControl<B>),
    Comm(FunderOutgoingComm<B>),
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum FunderOutgoingComm<B> {
    FriendMessage((PublicKey, FriendMessage<B>)),
    ChannelerConfig(ChannelerConfig<RelayAddress<B>>),
}
