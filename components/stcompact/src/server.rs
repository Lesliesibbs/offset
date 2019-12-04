use futures::{future, stream, StreamExt, channel::mpsc, SinkExt};

use common::select_streams::select_streams;
use common::conn::BoxStream;

use database::{DatabaseClient};

use app::conn::AppConnTuple;

use crate::persist::CompactState;
use crate::types::{CompactServerEvent, CompactServerState, CompactServerError, GenId, ConnPairCompact};

use crate::handle_user::handle_user;
use crate::handle_node::handle_node;
use crate::permission::check_permission;


/// The compact server is mediating between the user and the node.
async fn inner_server_loop<GI>(app_conn_tuple: AppConnTuple, 
    conn_pair_compact: ConnPairCompact, 
    compact_state: CompactState,
    database_client: DatabaseClient<CompactState>,
    mut gen_id: GI,
    mut opt_event_sender: Option<mpsc::Sender<()>>) -> Result<(), CompactServerError> 
where
    GI: GenId,
{

    // Interaction with the user:
    let (mut user_sender, user_receiver) = conn_pair_compact.split();
    let (app_permissions, node_report, conn_pair_app) = app_conn_tuple;
    // Interaction with the offst node:
    let (mut app_sender, app_receiver) = conn_pair_app.split();

    let user_receiver = user_receiver.map(CompactServerEvent::User)
        .chain(stream::once(future::ready(CompactServerEvent::UserClosed)));

    let app_receiver = app_receiver.map(CompactServerEvent::Node)
        .chain(stream::once(future::ready(CompactServerEvent::NodeClosed)));

    let mut incoming_events = select_streams![
        user_receiver,
        app_receiver
    ];

    let mut server_state = CompactServerState::new(node_report, compact_state, database_client);

    while let Some(event) = incoming_events.next().await {
        match event {
            CompactServerEvent::User(from_user) => {
                if check_permission(&from_user.user_request, &app_permissions) {
                    handle_user(from_user, &app_permissions, &mut server_state, &mut gen_id, &mut user_sender, &mut app_sender).await?;
                } else {
                    // Operation not permitted, we close the connection
                    return Ok(());
                }
            },
            CompactServerEvent::UserClosed => return Ok(()),
            CompactServerEvent::Node(app_server_to_app) => handle_node(app_server_to_app, &mut server_state, &mut gen_id, &mut user_sender, &mut app_sender).await?,
            CompactServerEvent::NodeClosed => return Ok(()),
        }
        if let Some(ref mut event_sender) = opt_event_sender {
            let _ = event_sender.send(()).await;
        }
    }
    Ok(())
}

#[allow(unused)]
pub async fn server_loop<GI>(app_conn_tuple: AppConnTuple, 
    conn_pair_compact: ConnPairCompact,
    compact_state: CompactState,
    database_client: DatabaseClient<CompactState>,
    gen_id: GI) -> Result<(), CompactServerError> 
where   
    GI: GenId,
{
    inner_server_loop(app_conn_tuple, conn_pair_compact, compact_state, database_client, gen_id, None).await
}
