use std::collections::{HashMap, HashSet};

use futures::channel::mpsc;
use futures::executor::ThreadPool;
use futures::task::{Spawn, SpawnExt};
use futures::{future, FutureExt, StreamExt, SinkExt};

use crypto::identity::{SoftwareEd25519Identity, generate_pkcs8_key_pair, 
    PublicKey};
use crypto::test_utils::DummyRandom;
use identity::{create_identity, IdentityClient};

use crate::funder::funder_loop;
use crate::types::{FunderOutgoingComm, IncomingCommMessage, 
    ChannelerConfig, FunderOutgoingControl, IncomingControlMessage,
    IncomingLivenessMessage};

#[derive(Debug)]
struct Node {
    friends: HashSet<PublicKey>,
    comm_out: mpsc::Sender<IncomingCommMessage>,
}

#[derive(Debug)]
struct NewNode<A> {
    public_key: PublicKey,
    comm_in: mpsc::Receiver<FunderOutgoingComm<A>>,
    comm_out: mpsc::Sender<IncomingCommMessage>,
}

#[derive(Debug)]
enum RouterEvent<A> {
    NewNode(NewNode<A>),
    OutgoingComm((PublicKey, FunderOutgoingComm<A>)), // (src_public_key, outgoing_comm)
}

async fn router_handle_outgoing_comm<A: 'static>(nodes: &mut HashMap<PublicKey, Node>, 
                                        src_public_key: PublicKey,
                                        outgoing_comm: FunderOutgoingComm<A>) {

    let node = nodes.get_mut(&src_public_key).unwrap();
    match outgoing_comm {
        FunderOutgoingComm::FriendMessage((dest_public_key, friend_message)) => {
            assert!(node.friends.contains(&dest_public_key));
            let incoming_comm_message = IncomingCommMessage::Friend((src_public_key.clone(), friend_message));
            await!(node.comm_out.send(incoming_comm_message)).unwrap();
        },
        FunderOutgoingComm::ChannelerConfig(channeler_config) => {
            match channeler_config {
                ChannelerConfig::AddFriend((friend_public_key, _address)) => {
                    assert!(node.friends.insert(friend_public_key.clone()));
                    let incoming_comm_message = IncomingCommMessage::Liveness(
                        IncomingLivenessMessage::Online(friend_public_key.clone()));
                    await!(node.comm_out.send(incoming_comm_message)).unwrap();
                },
                ChannelerConfig::RemoveFriend(friend_public_key) => {
                    assert!(node.friends.remove(&friend_public_key));
                    let incoming_comm_message = IncomingCommMessage::Liveness(
                        IncomingLivenessMessage::Offline(friend_public_key.clone()));
                    await!(node.comm_out.send(incoming_comm_message)).unwrap();
                },
            }
        },
    }
}

/// A future that forwards communication between nodes. Used for testing.
/// Simulates the Channeler interface
async fn router<A: Send + 'static>(incoming_new_node: mpsc::Receiver<NewNode<A>>, mut spawner: impl Spawn + Clone) {
    let mut nodes: HashMap<PublicKey, Node> = HashMap::new();
    let (comm_sender, comm_receiver) = mpsc::channel::<(PublicKey, FunderOutgoingComm<A>)>(0);

    let incoming_new_node = incoming_new_node
        .map(|new_node| RouterEvent::NewNode(new_node));
    let comm_receiver = comm_receiver
        .map(|tuple| RouterEvent::OutgoingComm(tuple));

    let mut events = incoming_new_node.select(comm_receiver);

    while let Some(event) = await!(events.next()) {
        match event {
            RouterEvent::NewNode(new_node) => {
                let NewNode {public_key, comm_in, comm_out} = new_node;
                nodes.insert(public_key.clone(), Node { friends: HashSet::new(), comm_out });

                let c_public_key = public_key.clone();
                let mut c_comm_sender = comm_sender.clone();
                let mut mapped_comm_in = comm_in.map(move |funder_outgoing_comm| 
                                                 (c_public_key.clone(), funder_outgoing_comm));
                let fut = async move {
                    await!(c_comm_sender.send_all(&mut mapped_comm_in))
                };
                spawner.spawn(fut.then(|_| future::ready(()))).unwrap();
            },
            RouterEvent::OutgoingComm((src_public_key, outgoing_comm)) => {
                router_handle_outgoing_comm(&mut nodes, src_public_key, outgoing_comm);
            }
        }
    }
}

struct NodeControl<A: Clone> {
    public_key: PublicKey,
    send_control: mpsc::Sender<IncomingControlMessage<A>>,
    recv_control: mpsc::Receiver<FunderOutgoingControl<A>>,
}

async fn task_funder_basic(identity_clients: Vec<IdentityClient>, 
                           mut spawner: impl Spawn + Clone + Send + 'static) {
    let (send_new_node, recv_new_node) = mpsc::channel::<NewNode<u32>>(0);
    spawner.spawn(router(recv_new_node, spawner.clone()));

    /*
    let (send_control, incoming_control) = mpsc::channel(0);
    let (control_sender, recv_control) = mpsc::channel(0);


    let node_controls = Vec::new();

    // Avoid problems with casting to u8:
    assert!(identity_clients.len() < 256);

    for i in 0 .. identity_clients.len() {
        let (send_comm, incoming_comm) = mpsc::channel(0);
        let (comm_sender, recv_comm) = mpsc::channel(0);

        let funder_fut = funder_loop(
            identity_client[i].clone(),
            DummyRandom::new(&[i as u8]),
            incoming_control,
            incoming_comm,
            control_sender,
            comm_sender,
            db_core);

        // Add back when we know what to do with db_core.

        node_controls.push(NodeControl {
            public_key: await!(identity_clients[i].request_public_key()).unwrap(),
            send_control,
            recv_control,
        });
    }
    */

}

#[test]
fn test_funder_basic() {
    let mut thread_pool = ThreadPool::new().unwrap();

    let mut identity_clients = Vec::new();

    for i in 0 .. 6u8 {
        let rng = DummyRandom::new(&[i]);
        let pkcs8 = generate_pkcs8_key_pair(&rng);
        let identity1 = SoftwareEd25519Identity::from_pkcs8(&pkcs8).unwrap();
        let (requests_sender, identity_server) = create_identity(identity1);
        let identity_client = IdentityClient::new(requests_sender);
        thread_pool.spawn(identity_server.then(|_| future::ready(()))).unwrap();
        identity_clients.push(identity_client);
    }

    thread_pool.run(task_funder_basic(identity_clients, thread_pool.clone()));
}
