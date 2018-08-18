use crypto::identity::verify_signature;

use utils::int_convert::usize_to_u32;
use utils::safe_arithmetic::SafeArithmetic;

use super::super::types::{ResponseSendFunds, FailureSendFunds, RequestSendFunds,
                          FriendTcOp, PendingFriendRequest };

use super::super::credit_calc::CreditCalculator;
use super::super::signature_buff::{create_response_signature_buffer, 
    verify_failure_signature};

use super::types::{TokenChannel, MAX_FUNDER_DEBT, TcMutation};
use super::super::messages::RequestsStatus;


/*
pub struct IncomingRequestSendFunds {
    pub request: PendingFriendRequest,
}
*/

pub struct IncomingResponseSendFunds {
    pub pending_request: PendingFriendRequest,
    pub incoming_response: ResponseSendFunds,
}

pub struct IncomingFailureSendFunds {
    pub pending_request: PendingFriendRequest,
    pub incoming_failure: FailureSendFunds,
}

pub enum IncomingMessage {
    Request(RequestSendFunds),
    Response(IncomingResponseSendFunds),
    Failure(IncomingFailureSendFunds),
}

/// Resulting tasks to perform after processing an incoming operation.
#[allow(unused)]
pub struct ProcessOperationOutput {
    pub incoming_message: Option<IncomingMessage>,
    pub tc_mutations: Vec<TcMutation>,
}


#[derive(Debug)]
pub enum ProcessOperationError {
    RemoteMaxDebtTooLarge(u128),
    /// Trying to set the invoiceId, while already expecting another invoice id.
    PkPairNotInRoute,
    /// The Route contains some public key twice.
    DuplicateNodesInRoute,
    RequestsAlreadyDisabled,
    RouteTooLong,
    InsufficientTrust,
    CreditsCalcOverflow,
    InvalidFreezeLinks,
    CreditCalculatorFailure,
    RequestAlreadyExists,
    RequestDoesNotExist,
    InvalidResponseSignature,
    ReportingNodeNonexistent,
    InvalidReportingNode,
    InvalidFailureSignature,
    LocalRequestsClosed,
}

#[derive(Debug)]
pub struct ProcessTransListError {
    index: usize,
    process_trans_error: ProcessOperationError,
}


pub fn simulate_process_operations_list(token_channel: &TokenChannel, 
                                        operations: Vec<FriendTcOp>) ->
    Result<Vec<ProcessOperationOutput>, ProcessTransListError> {

    let mut outputs = Vec::new();

    // We do not change the original TokenChannel. 
    // Instead, we are operating over a clone:
    // This operation is not very expensive, because we are using immutable data structures
    // (specifically, HashMaps).
    let mut cloned_token_channel = token_channel.clone();

    for (index, funds) in operations.into_iter().enumerate() {
        match process_operation(&mut cloned_token_channel, funds) {
            Err(e) => return Err(ProcessTransListError {
                index,
                process_trans_error: e
            }),
            Ok(trans_output) => outputs.push(trans_output),
        }
    }
    Ok(outputs)
}

fn process_operation(token_channel: &mut TokenChannel, funds: FriendTcOp) ->
    Result<ProcessOperationOutput, ProcessOperationError> {
    match funds {
        FriendTcOp::EnableRequests =>
            process_enable_requests(token_channel),
        FriendTcOp::DisableRequests =>
            process_disable_requests(token_channel),
        FriendTcOp::SetRemoteMaxDebt(proposed_max_debt) =>
            process_set_remote_max_debt(token_channel, proposed_max_debt),
        FriendTcOp::RequestSendFunds(request_send_funds) =>
            process_request_send_funds(token_channel, request_send_funds),
        FriendTcOp::ResponseSendFunds(response_send_funds) =>
            process_response_send_funds(token_channel, response_send_funds),
        FriendTcOp::FailureSendFunds(failure_send_funds) =>
            process_failure_send_funds(token_channel, failure_send_funds),
    }
}

fn process_enable_requests(token_channel: &mut TokenChannel) ->
    Result<ProcessOperationOutput, ProcessOperationError> {

    let mut op_output = ProcessOperationOutput {
        incoming_message: None,
        tc_mutations: Vec::new(),
    };
    let tc_mutation = TcMutation::SetRemoteRequestsStatus(RequestsStatus::Open);
    token_channel.mutate(&tc_mutation);
    op_output.tc_mutations.push(tc_mutation);

    Ok(op_output)
}

fn process_disable_requests(token_channel: &mut TokenChannel) ->
    Result<ProcessOperationOutput, ProcessOperationError> {

    let mut op_output = ProcessOperationOutput {
        incoming_message: None,
        tc_mutations: Vec::new(),
    };

    match token_channel.state().requests_status.remote {
        RequestsStatus::Open => {
            let tc_mutation = TcMutation::SetRemoteRequestsStatus(RequestsStatus::Closed);
            token_channel.mutate(&tc_mutation);
            op_output.tc_mutations.push(tc_mutation);
            Ok(op_output)
        },
        RequestsStatus::Closed => Err(ProcessOperationError::RequestsAlreadyDisabled),
    }
}

fn process_set_remote_max_debt(token_channel: &mut TokenChannel,
                               proposed_max_debt: u128) -> 
    Result<ProcessOperationOutput, ProcessOperationError> {

    let mut op_output = ProcessOperationOutput {
        incoming_message: None,
        tc_mutations: Vec::new(),
    };

    if proposed_max_debt > MAX_FUNDER_DEBT {
        Err(ProcessOperationError::RemoteMaxDebtTooLarge(proposed_max_debt))
    } else {
        let tc_mutation = TcMutation::SetLocalMaxDebt(proposed_max_debt);
        token_channel.mutate(&tc_mutation);
        op_output.tc_mutations.push(tc_mutation);
        Ok(op_output)
    }
}


/// Process an incoming RequestSendFunds
fn process_request_send_funds(token_channel: &mut TokenChannel,
                                request_send_funds: RequestSendFunds)
    -> Result<ProcessOperationOutput, ProcessOperationError> {

    // Make sure that the route does not contains cycles/duplicates:
    if !request_send_funds.route.is_cycle_free() {
        return Err(ProcessOperationError::DuplicateNodesInRoute);
    }

    // Find ourselves on the route. If we are not there, abort.
    let remote_index = request_send_funds.route.find_pk_pair(
        &token_channel.state().idents.remote_public_key, 
        &token_channel.state().idents.local_public_key)
        .ok_or(ProcessOperationError::PkPairNotInRoute)?;

    // Make sure that freeze_links and route_links are compatible in length:
    let freeze_links_len = request_send_funds.freeze_links.len();
    if remote_index.checked_add(1).unwrap() != freeze_links_len {
        return Err(ProcessOperationError::InvalidFreezeLinks);
    }

    // Make sure that we are open to requests:
    if let RequestsStatus::Open = token_channel.state().requests_status.local {
        return Err(ProcessOperationError::LocalRequestsClosed);
    }

    let route_len = usize_to_u32(request_send_funds.route.len())
        .ok_or(ProcessOperationError::RouteTooLong)?;
    let credit_calc = CreditCalculator::new(route_len,
                                            request_send_funds.dest_payment);

    let local_index = remote_index.checked_add(1)
        .ok_or(ProcessOperationError::RouteTooLong)?;
    let local_index = usize_to_u32(local_index)
        .ok_or(ProcessOperationError::RouteTooLong)?;

    // Calculate amount of credits to freeze
    let own_freeze_credits = credit_calc.credits_to_freeze(local_index)
        .ok_or(ProcessOperationError::CreditCalculatorFailure)?;

    // Make sure we can freeze the credits
    let new_remote_pending_debt = token_channel.state().balance.remote_pending_debt
        .checked_add(own_freeze_credits).ok_or(ProcessOperationError::CreditsCalcOverflow)?;

    if new_remote_pending_debt > token_channel.state().balance.remote_max_debt {
        return Err(ProcessOperationError::InsufficientTrust);
    }

    // Note that Verifying our own freezing link will be done outside. We don't have enough
    // information here to check this. In addition, even if it turns out we can't freeze those
    // credits, we don't want to create a token channel inconsistency.         
    
    let p_remote_requests = &token_channel.state().pending_requests.pending_remote_requests;
    // Make sure that we don't have this request as a pending request already:
    if p_remote_requests.contains_key(&request_send_funds.request_id) {
        return Err(ProcessOperationError::RequestAlreadyExists);
    }

    // Add pending request funds:
    let pending_friend_request = request_send_funds.create_pending_request();

    let mut op_output = ProcessOperationOutput {
        incoming_message: Some(IncomingMessage::Request(request_send_funds)),
        tc_mutations: Vec::new(),
    };

    let tc_mutation = TcMutation::InsertRemotePendingRequest(pending_friend_request);
    token_channel.mutate(&tc_mutation);
    op_output.tc_mutations.push(tc_mutation);

    // If we are here, we can freeze the credits:
    let tc_mutation = TcMutation::SetRemotePendingDebt(new_remote_pending_debt);
    token_channel.mutate(&tc_mutation);
    op_output.tc_mutations.push(tc_mutation);

    Ok(op_output)

}

fn process_response_send_funds(token_channel: &mut TokenChannel,
                                 response_send_funds: ResponseSendFunds) ->
    Result<ProcessOperationOutput, ProcessOperationError> {

    // Make sure that id exists in local_pending hashmap, 
    // and access saved request details.
    let local_pending_requests = &token_channel.state().pending_requests.pending_local_requests;

    // Obtain pending request:
    // TODO: Possibly get rid of clone() here for optimization later
    let pending_request = local_pending_requests
        .get(&response_send_funds.request_id)
        .ok_or(ProcessOperationError::RequestDoesNotExist)?
        .clone();

    let response_signature_buffer = create_response_signature_buffer(
                                        &response_send_funds,
                                        &pending_request);

    // Verify response funds signature:
    if !verify_signature(&response_signature_buffer, 
                             &token_channel.state().idents.remote_public_key,
                             &response_send_funds.signature) {
        return Err(ProcessOperationError::InvalidResponseSignature);
    }

    // It should never happen that usize_to_u32 fails here, because we 
    // checked this when we created the pending_request.
    let route_len = usize_to_u32(pending_request.route.len()).unwrap();
    let credit_calc = CreditCalculator::new(route_len,
                                            pending_request.dest_payment);

    // Find ourselves on the route. If we are not there, abort.
    let local_index = pending_request.route.find_pk_pair(
        &token_channel.state().idents.local_public_key, 
        &token_channel.state().idents.remote_public_key).unwrap();

    let mut tc_mutations = Vec::new();

    // Remove entry from local_pending hashmap:
    let tc_mutation = TcMutation::RemoveLocalPendingRequest(response_send_funds.request_id);
    token_channel.mutate(&tc_mutation);
    tc_mutations.push(tc_mutation);

    let remote_index = usize_to_u32(local_index.checked_add(1).unwrap()).unwrap();
    let success_credits = credit_calc.credits_on_success(remote_index).unwrap();
    let freeze_credits = credit_calc.credits_to_freeze(remote_index).unwrap();

    // Decrease frozen credits and decrease balance:
    let new_local_pending_debt = 
        token_channel.state().balance.local_pending_debt
        .checked_sub(freeze_credits)
        .unwrap();

    let tc_mutation = TcMutation::SetLocalPendingDebt(new_local_pending_debt);
    token_channel.mutate(&tc_mutation);
    tc_mutations.push(tc_mutation);

    let new_balance = 
        token_channel.state().balance.balance
        .checked_sub_unsigned(success_credits)
        .unwrap();

    let tc_mutation = TcMutation::SetBalance(new_balance);
    token_channel.mutate(&tc_mutation);
    tc_mutations.push(tc_mutation);

    let incoming_message = Some(
        IncomingMessage::Response(
            IncomingResponseSendFunds {
                pending_request,
                incoming_response: response_send_funds,
            }
        )
    );

    Ok(ProcessOperationOutput {
        incoming_message,
        tc_mutations,
    })
}

fn process_failure_send_funds(token_channel: &mut TokenChannel,
                                failure_send_funds: FailureSendFunds) ->
    Result<ProcessOperationOutput, ProcessOperationError> {
    
    // Make sure that id exists in local_pending hashmap, 
    // and access saved request details.
    let local_pending_requests = &token_channel.state().pending_requests.pending_local_requests;

    // Obtain pending request:
    let pending_request = local_pending_requests
        .get(&failure_send_funds.request_id)
        .ok_or(ProcessOperationError::RequestDoesNotExist)?
        .clone();
    // TODO: Possibly get rid of clone() here for optimization later

    // Find ourselves on the route. If we are not there, abort.
    let local_index = pending_request.route.find_pk_pair(
        &token_channel.state().idents.local_public_key, 
        &token_channel.state().idents.remote_public_key).unwrap();

    // Make sure that reporting node public key is:
    //  - inside the route
    //  - After us on the route.
    //  - Not the destination node
    
    let reporting_index = pending_request.route.pk_to_index(
        &failure_send_funds.reporting_public_key)
        .ok_or(ProcessOperationError::ReportingNodeNonexistent)?;

    if reporting_index <= local_index {
        return Err(ProcessOperationError::InvalidReportingNode);
    }


    verify_failure_signature(&failure_send_funds, &pending_request)
        .ok_or(ProcessOperationError::InvalidFailureSignature)?;

    // At this point we believe the failure funds is valid.
    let route_len = usize_to_u32(pending_request.route.len()).unwrap();
    let credit_calc = CreditCalculator::new(route_len,
                                            pending_request.dest_payment);

    let mut tc_mutations = Vec::new();

    // Remove entry from local_pending hashmap:
    let tc_mutation = TcMutation::RemoveLocalPendingRequest(failure_send_funds.request_id);
    token_channel.mutate(&tc_mutation);
    tc_mutations.push(tc_mutation);


    let remote_index = usize_to_u32(local_index.checked_add(1).unwrap()).unwrap();
    let reporting_index = usize_to_u32(reporting_index).unwrap();
    let failure_credits = credit_calc.credits_on_failure(remote_index, reporting_index)
        .unwrap();
    let freeze_credits = credit_calc.credits_to_freeze(remote_index)
        .unwrap();

    // Decrease frozen credits and decrease balance:
    let new_local_pending_debt = 
        token_channel.state().balance.local_pending_debt.checked_sub(freeze_credits)
        .unwrap();

    let tc_mutation = TcMutation::SetLocalPendingDebt(new_local_pending_debt);
    token_channel.mutate(&tc_mutation);
    tc_mutations.push(tc_mutation);

    let new_balance = 
        token_channel.state().balance.balance.checked_sub_unsigned(failure_credits)
        .unwrap();

    let tc_mutation = TcMutation::SetBalance(new_balance);
    token_channel.mutate(&tc_mutation);
    tc_mutations.push(tc_mutation);
    
    // Return Failure funds.
    let incoming_message = Some(
        IncomingMessage::Failure(
            IncomingFailureSendFunds {
                pending_request,
                incoming_failure: failure_send_funds,
            }
        )
    );

    Ok(ProcessOperationOutput {
        incoming_message,
        tc_mutations,
    })

}
