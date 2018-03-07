use std::cmp;
use crypto::identity::PublicKey;


/// Amount of credits paid to destination node, upon issuing a signed Response message.
/// The destination node is the last node along the route of a request.
/// Upon any overflow (u64) this function will return None.
fn credits_on_success_dest(processing_fee_proposal: u64, request_len: u32, credits_per_byte_proposal: u64,
                           response_len: u32, max_response_len: u32) -> Option<u64> {

    // processing_fee_proposal + request_len * creditsPerBytesProposal + (max_response_len - response_len)
    processing_fee_proposal.checked_add(
    (request_len as u64).checked_mul(credits_per_byte_proposal)?
    )?.checked_add(
        (max_response_len as u64).checked_sub(response_len as u64)?
    )
}


/// Amount of credit paid to a node that sent a valid Response (Which closes an open request).
/// This amount depends upon the original request, and also on the position of the node along the
/// route used for sending the request (This is represented by the nodes_to_dest argument).
/// credits_on_success_dest is a special case of this function, for nodes_to_dest = 0.
///
/// Example:
/// A -- B -- (C) -- D -- E
///
/// The node C has nodes_to_dest = 2.
/// This function calculates the amount of credits C should obtain if he sent back a signed
/// Response message to B, as a response to a Request sent from A all the way to E.
/// Upon any overflow (u64) this function will return None.
pub fn credits_on_success(processing_fee_proposal: u64, request_len: u32, 
                     credits_per_byte_proposal: u64, max_response_len: u32, 
                     response_len: u32, nodes_to_dest: usize) -> Option<u64> {

    // (request_len + response_len) * credits_per_byte_proposal * nodes_to_dest + 
    //      credits_on_success_dest(...)
    (request_len as u64).checked_add(response_len as u64)?
        .checked_mul(credits_per_byte_proposal)?
        .checked_mul(nodes_to_dest as u64)?
        .checked_add(
            credits_on_success_dest(processing_fee_proposal, 
                                    request_len, 
                                    credits_per_byte_proposal,
                                    response_len,
                                    max_response_len)?)
}

/// The amount of credits paid to a node in case of failure.
/// This amount depends on the length of the Request message, 
/// and also on the amount of nodes until the reporting node.
/// Example:
///
/// A -- B -- C -- D -- E
///
/// Asssume that A sends a Request message along the route in the picture all the way to E.
/// Assume that D is not willing to pass the message to E for some reason, and therefore he reports
/// a failure message back to C. In this case, for example: 
/// - D will receive credits_on_failure(request_len, 0) credits
/// - C will receive credits_on_failure(request_len, 1) credits.
pub fn credits_on_failure(request_len: u32, nodes_to_reporting: usize) -> Option<u64> {
    // request_len * (nodes_to_reporting + 1)
    (request_len as u64).checked_mul((nodes_to_reporting as u64).checked_add(1)?)
}

/// Compute the amount of credits we need to freeze on a node along a request route. 
/// Example:
///
/// A -- B -- (C) -- D -- E
///
/// A sends a request along the route in the picture, all the way to E.  Here we can compute for
/// example the maximum amount of credits C has to freeze when the request goes through C to D. The
/// amount of credits to freeze should be the maximum amount of credits C can earn.
pub fn credits_to_freeze(processing_fee_proposal: u64, request_len: u32,
                         credits_per_byte_proposal: u64, max_response_len: u32,
                         nodes_to_dest: usize) -> Option<u64> {

    // Note: Here we take the maximum for credits_on_success for the cases of:  
    // - resposne_len = 0
    // - response_len = max_response_len.  
    // We do this because credits_on_success is linear with respect to the response_len argument,
    // hence the maximum of credits_on_success must be on one of the edges.
    
    let credits_resp_len_zero = credits_on_success(processing_fee_proposal, 
                       request_len,
                       credits_per_byte_proposal, 
                       max_response_len,
                       0,               // Minimal response_len
                       nodes_to_dest)?;
    let credits_resp_len_max = credits_on_success(processing_fee_proposal, 
                       request_len,
                       credits_per_byte_proposal, 
                       max_response_len,
                       max_response_len, // Maximum response len
                       nodes_to_dest)?;

    Some(cmp::min(credits_resp_len_zero, credits_resp_len_max))
}

