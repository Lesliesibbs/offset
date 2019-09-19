use std::time::Duration;

use futures::channel::mpsc;
use futures::executor::ThreadPool;
use futures::task::{Spawn, SpawnExt};
use futures::{SinkExt, StreamExt};

use common::conn::{ConnPairVec, FutTransform};
use common::int_convert::usize_to_u64;

use proto::app_server::messages::{AppPermissions, AppServerToApp, AppToAppServer};
/*
use proto::app_server::serialize::{
    deserialize_app_permissions, deserialize_app_server_to_app, serialize_app_to_app_server,
};
*/
use proto::consts::{KEEPALIVE_TICKS, PROTOCOL_VERSION, TICKS_TO_REKEY};
use proto::consts::{MAX_FRAME_LENGTH, TICK_MS};
use proto::net::messages::NetAddress;
use proto::proto_ser::{ProtoDeserialize, ProtoSerialize};

use proto::crypto::PublicKey;

use crypto::rand::{system_random, CryptoRandom};

use identity::IdentityClient;
use net::NetConnector;
use timer::create_timer;

use timer::TimerClient;

pub use super::app_conn::AppConn;
use super::app_conn::AppConnTuple;

use keepalive::KeepAliveChannel;
use secure_channel::SecureChannel;
use version::VersionPrefix;

#[derive(Debug)]
pub enum SetupConnectionError {
    EncryptSetupError,
    RecvAppPermissionsError,
    DeserializeAppPermissionsError,
    ClosedBeforeNodeReport,
    DeserializeNodeReportError,
    FirstMessageNotNodeReport,
}

/// Connect to an offst-node
pub async fn setup_connection<R, S>(
    conn_pair: ConnPairVec,
    timer_client: TimerClient,
    rng: R,
    node_public_key: PublicKey,
    app_identity_client: IdentityClient,
    mut spawner: S,
) -> Result<AppConnTuple, SetupConnectionError>
where
    R: Clone + CryptoRandom + 'static,
    S: Spawn + Clone + Send + Sync + 'static,
{
    let mut version_transform = VersionPrefix::new(PROTOCOL_VERSION, spawner.clone());

    let mut encrypt_transform = SecureChannel::new(
        app_identity_client.clone(),
        rng.clone(),
        timer_client.clone(),
        TICKS_TO_REKEY,
        spawner.clone(),
    );

    let mut keepalive_transform =
        KeepAliveChannel::new(timer_client.clone(), KEEPALIVE_TICKS, spawner.clone());

    // Report version and check remote side's version:
    let ver_conn = version_transform.transform(conn_pair).await;

    // Encrypt, requiring that the remote side will have node_public_key as public key:
    let (public_key, enc_conn) =
        encrypt_transform.transform((Some(node_public_key.clone()), ver_conn)).await
            .ok_or(SetupConnectionError::EncryptSetupError)?;
    assert_eq!(public_key, node_public_key);

    // Keepalive wrapper:
    let (mut sender, mut receiver) = keepalive_transform.transform(enc_conn).await;

    // Get AppPermissions:
    let app_permissions_data =
        receiver.next().await.ok_or(SetupConnectionError::RecvAppPermissionsError)?;
    let app_permissions = AppPermissions::proto_deserialize(&app_permissions_data)
        .map_err(|_| SetupConnectionError::DeserializeAppPermissionsError)?;

    // Wait for the first NodeReport.
    let data = receiver.next().await.ok_or(SetupConnectionError::ClosedBeforeNodeReport)?;
    let message = AppServerToApp::proto_deserialize(&data)
        .map_err(|_| SetupConnectionError::DeserializeNodeReportError)?;

    let node_report = if let AppServerToApp::Report(node_report) = message {
        node_report
    } else {
        return Err(SetupConnectionError::FirstMessageNotNodeReport)
    };

    // serialization:
    let (user_sender, mut from_user_sender) = mpsc::channel::<AppToAppServer>(0);
    let (mut to_user_receiver, user_receiver) = mpsc::channel(0);

    // Deserialize data received from node:
    let _ = spawner.spawn(async move {
        while let Some(data) = receiver.next().await {
            let message = match AppServerToApp::proto_deserialize(&data) {
                Ok(message) => message,
                Err(_) => return,
            };
            if to_user_receiver.send(message).await.is_err() {
                return;
            }
        }
    });

    // Serialize data sent to node:
    let _ = spawner.spawn(async move {
        while let Some(message) = from_user_sender.next().await {
            // let data = serialize_app_to_app_server(&message);
            let data = message.proto_serialize();
            if sender.send(data).await.is_err() {
                return;
            }
        }
    });

    Ok((app_permissions, node_report, (user_sender, user_receiver)))
}

#[derive(Debug)]
pub enum NodeConnectError {
    /// Could not open network connection
    NetConnectorError,
    SetupConnectionError(SetupConnectionError),
    CreateNodeConnectionError,
}

/// Connect to an offst node
pub async fn node_connect<C, R, S>(
    mut net_connector: C,
    node_public_key: PublicKey,
    node_net_address: NetAddress,
    timer_client: TimerClient,
    app_identity_client: IdentityClient,
    rng: R,
    mut spawner: S,
) -> Result<AppConn<R>, NodeConnectError>
where
    C: FutTransform<Input = NetAddress, Output = Option<ConnPairVec>>,
    R: CryptoRandom + Clone + 'static,
    S: Spawn + Send + Sync + Clone + 'static,
{
    let conn_pair = net_connector.transform(node_net_address).await
        .ok_or(NodeConnectError::NetConnectorError)?;

    let conn_tuple = setup_connection(
        conn_pair,
        timer_client,
        rng.clone(),
        node_public_key,
        app_identity_client,
        spawner.clone()
    ).await
    .map_err(NodeConnectError::SetupConnectionError)?;

    AppConn::new(conn_tuple, rng, &mut spawner)
        .map_err(|_| NodeConnectError::CreateNodeConnectionError)
}

#[derive(Debug)]
pub struct ConnectError;

/// Connect to a remote offst-node.
pub async fn connect<S>(
    node_public_key: PublicKey,
    node_net_address: NetAddress,
    app_identity_client: IdentityClient,
    spawner: S,
) -> Result<AppConn, ConnectError>
where
    S: Spawn + Clone + Send + Sync + 'static,
{
    let resolve_thread_pool = ThreadPool::new().map_err(|_| ConnectError)?;

    // A tcp connector, Used to connect to remote servers:
    let net_connector = NetConnector::new(MAX_FRAME_LENGTH, resolve_thread_pool, spawner.clone());

    // Get a timer client:
    let dur = Duration::from_millis(usize_to_u64(TICK_MS).unwrap());
    let timer_client = create_timer(dur, spawner.clone()).map_err(|_| ConnectError)?;

    // Obtain secure cryptographic random:
    let rng = system_random();

    node_connect(
        net_connector,
        node_public_key,
        node_net_address,
        timer_client,
        app_identity_client,
        rng,
        spawner
    ).await
    .map_err(|_| ConnectError)
}
