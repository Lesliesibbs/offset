use futures::channel::mpsc;
use futures::executor::ThreadPool;
use futures::task::Spawn;
use futures::{SinkExt, StreamExt};

use proto::crypto::PublicKey;

use proto::app_server::messages::{AppPermissions, AppServerToApp, NodeReportMutation};
use proto::index_client::messages::{
    IndexClientReportMutation, IndexClientReportMutations, IndexClientToAppServer,
};
use proto::index_server::messages::NamedIndexServerAddress;

use super::utils::spawn_dummy_app_server;

async fn task_app_server_loop_two_apps<S>(spawner: S)
where
    S: Spawn + Clone + Send + 'static,
{
    let (
        _funder_sender,
        _funder_receiver,
        mut index_client_sender,
        _index_client_receiver,
        mut connections_sender,
        initial_node_report,
    ) = spawn_dummy_app_server(spawner.clone());

    let (_app_sender0, app_server_receiver) = mpsc::channel(0);
    let (app_server_sender, mut app_receiver0) = mpsc::channel(0);
    let app_server_conn_pair = (app_server_sender, app_server_receiver);
    let app_permissions = AppPermissions {
        routes: true,
        buyer: true,
        seller: true,
        config: true,
    };
    connections_sender.send((app_permissions, app_server_conn_pair)).await.unwrap();

    let (_app_sender1, app_server_receiver) = mpsc::channel(0);
    let (app_server_sender, mut app_receiver1) = mpsc::channel(0);
    let app_server_conn_pair = (app_server_sender, app_server_receiver);
    let app_permissions = AppPermissions {
        routes: true,
        buyer: true,
        seller: true,
        config: true,
    };
    connections_sender.send((app_permissions, app_server_conn_pair)).await.unwrap();

    // The apps should receive the current node report as the first message:
    // Send a report
    let to_app_message = app_receiver0.next().await.unwrap();
    match to_app_message {
        AppServerToApp::Report(report) => assert_eq!(report, initial_node_report),
        _ => unreachable!(),
    };
    let to_app_message = app_receiver1.next().await.unwrap();
    match to_app_message {
        AppServerToApp::Report(report) => assert_eq!(report, initial_node_report),
        _ => unreachable!(),
    };

    // Send a dummy report message from IndexClient:
    let named_relay_server_address = NamedIndexServerAddress {
        public_key: PublicKey::from(&[0xaa; PublicKey::len()]),
        address: 300u32,
        name: "IndexServer300".to_string(),
    };
    let index_client_report_mutation =
        IndexClientReportMutation::AddIndexServer(named_relay_server_address.clone());
    let mutations = vec![index_client_report_mutation.clone()];
    let index_client_report_mutations = IndexClientReportMutations {
        opt_app_request_id: None,
        mutations,
    };
    index_client_sender.send(IndexClientToAppServer::ReportMutations(
        index_client_report_mutations
    ))
    .await
    .unwrap();

    // Both apps should get the report:
    let to_app_message = app_receiver0.next().await.unwrap();
    match to_app_message {
        AppServerToApp::ReportMutations(report_mutations) => {
            assert!(report_mutations.opt_app_request_id.is_none());
            assert_eq!(report_mutations.mutations.len(), 1);
            let report_mutation = &report_mutations.mutations[0];
            match report_mutation {
                NodeReportMutation::IndexClient(received_index_client_report_mutation) => {
                    assert_eq!(
                        received_index_client_report_mutation,
                        &index_client_report_mutation
                    );
                }
                _ => unreachable!(),
            }
        }
        _ => unreachable!(),
    }
    let to_app_message = app_receiver1.next().await.unwrap();
    match to_app_message {
        AppServerToApp::ReportMutations(report_mutations) => {
            assert_eq!(report_mutations.mutations.len(), 1);
            let report_mutation = &report_mutations.mutations[0];
            match report_mutation {
                NodeReportMutation::IndexClient(received_index_client_report_mutation) => {
                    assert_eq!(
                        received_index_client_report_mutation,
                        &index_client_report_mutation
                    );
                }
                _ => unreachable!(),
            }
        }
        _ => unreachable!(),
    }
}

#[test]
fn test_app_server_loop_index_two_apps() {
    let mut thread_pool = ThreadPool::new().unwrap();
    thread_pool.run(task_app_server_loop_two_apps(thread_pool.clone()));
}
