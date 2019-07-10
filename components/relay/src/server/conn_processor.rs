use std::marker::Unpin;

use futures::channel::mpsc;
use futures::{future, Sink, SinkExt, Stream, StreamExt};

use common::conn::{ConnPairVec, FutTransform};

use timer::utils::future_timeout;
use timer::TimerClient;

use super::types::{
    IncomingAccept, IncomingConn, IncomingConnInner, IncomingConnect, IncomingListen,
};

use proto::crypto::PublicKey;
use proto::relay::messages::{IncomingConnection, InitConnection, RejectConnection};
use proto::proto_ser::{ProtoSerialize, ProtoDeserialize};

/*
use proto::relay::serialize::{
    deserialize_init_connection, deserialize_reject_connection, serialize_incoming_connection,
};
*/

async fn dispatch_conn<FT>(
    sender: mpsc::Sender<Vec<u8>>,
    receiver: mpsc::Receiver<Vec<u8>>,
    public_key: PublicKey,
    first_msg: Vec<u8>,
    mut keepalive_transform: FT,
) -> Option<
    IncomingConn<
        impl Stream<Item = RejectConnection> + Unpin,
        impl Sink<IncomingConnection, SinkError = ()> + Unpin,
        impl Stream<Item = Vec<u8>> + Unpin,
        impl Sink<Vec<u8>, SinkError = ()> + Unpin,
        impl Stream<Item = Vec<u8>> + Unpin,
        impl Sink<Vec<u8>, SinkError = ()> + Unpin,
    >,
>
where
    FT: FutTransform<Input = ConnPairVec, Output = ConnPairVec>,
{
    let (sender, receiver) = await!(keepalive_transform.transform((sender, receiver)));

    let sender = sender.sink_map_err(|_| ());
    let inner = match InitConnection::proto_deserialize(&first_msg).ok()? {
        InitConnection::Listen => IncomingConnInner::Listen(IncomingListen {
            receiver: receiver
                .map(|data| RejectConnection::proto_deserialize(&data))
                .take_while(|res| future::ready(res.is_ok()))
                .map(Result::unwrap),
            sender: sender.with(|msg: IncomingConnection| future::ready(Ok(msg.proto_serialize()))),
        }),
        InitConnection::Accept(accept_public_key) => IncomingConnInner::Accept(IncomingAccept {
            receiver,
            sender,
            accept_public_key,
        }),
        InitConnection::Connect(connect_public_key) => {
            IncomingConnInner::Connect(IncomingConnect {
                receiver,
                sender,
                connect_public_key,
            })
        }
    };

    Some(IncomingConn { public_key, inner })
}

async fn process_conn<FT>(
    sender: mpsc::Sender<Vec<u8>>,
    mut receiver: mpsc::Receiver<Vec<u8>>,
    public_key: PublicKey,
    keepalive_transform: FT,
    mut timer_client: TimerClient,
    conn_timeout_ticks: usize,
) -> Option<
    IncomingConn<
        impl Stream<Item = RejectConnection> + Unpin,
        impl Sink<IncomingConnection, SinkError = ()> + Unpin,
        impl Stream<Item = Vec<u8>> + Unpin,
        impl Sink<Vec<u8>, SinkError = ()> + Unpin,
        impl Stream<Item = Vec<u8>> + Unpin,
        impl Sink<Vec<u8>, SinkError = ()> + Unpin,
    >,
>
where
    FT: FutTransform<Input = ConnPairVec, Output = ConnPairVec>,
{
    let fut_receiver = Box::pin(async move {
        if let Some(first_msg) = await!(receiver.next()) {
            let dispatch_res = await!(dispatch_conn(
                sender,
                receiver,
                public_key,
                first_msg,
                keepalive_transform
            ));
            if dispatch_res.is_none() {
                warn!("process_conn(): dispatch_conn() failure");
            }
            dispatch_res
        } else {
            None
        }
    });

    let timer_stream = await!(timer_client.request_timer_stream()).unwrap();
    let res = await!(future_timeout(
        fut_receiver,
        timer_stream,
        conn_timeout_ticks
    ))?;
    if res.is_none() {
        warn!("process_conn(): timeout occurred");
    }
    res
}

/// Process incoming connections
/// For each connection obtain the first message, and prepare the correct type according to this
/// first messages.
/// If waiting for the first message takes too long, discard the connection.
pub fn conn_processor<T, FT>(
    incoming_conns: T,
    keepalive_transform: FT,
    timer_client: TimerClient,
    conn_timeout_ticks: usize,
) -> impl Stream<
    Item = IncomingConn<
        impl Stream<Item = RejectConnection>,
        impl Sink<IncomingConnection, SinkError = ()>,
        impl Stream<Item = Vec<u8>>,
        impl Sink<Vec<u8>, SinkError = ()>,
        impl Stream<Item = Vec<u8>>,
        impl Sink<Vec<u8>, SinkError = ()>,
    >,
>
where
    T: Stream<Item = (PublicKey, ConnPairVec)> + Unpin,
    FT: FutTransform<Input = ConnPairVec, Output = ConnPairVec> + Clone,
{
    incoming_conns
        .map(move |(public_key, (sender, receiver))| {
            process_conn(
                sender,
                receiver,
                public_key,
                keepalive_transform.clone(),
                timer_client.clone(),
                conn_timeout_ticks,
            )
        })
        .filter_map(|opt_conn| opt_conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    use futures::channel::mpsc;
    use futures::executor::ThreadPool;
    use futures::task::{Spawn, SpawnExt};
    use futures::{stream, FutureExt};

    use common::async_test_utils::receive;
    use common::conn::FuncFutTransform;
    use proto::crypto::{PublicKey, PUBLIC_KEY_LEN};
    use timer::create_timer_incoming;


    async fn task_dispatch_conn_basic(spawner: impl Spawn + Clone) {
        // Create a mock time service:
        let (_tick_sender, tick_receiver) = mpsc::channel::<()>(0);
        let _timer_client = create_timer_incoming(tick_receiver, spawner.clone()).unwrap();

        let (sender, receiver) = mpsc::channel::<Vec<u8>>(0);
        let first_msg = InitConnection::Listen;
        let ser_first_msg = first_msg.proto_serialize();
        let public_key = PublicKey::from(&[0x77; PUBLIC_KEY_LEN]);
        let keepalive_transform = FuncFutTransform::new(|x| Box::pin(future::ready(x)));
        let incoming_conn = await!(dispatch_conn(
            sender,
            receiver,
            public_key.clone(),
            ser_first_msg,
            keepalive_transform
        ))
        .unwrap();

        assert_eq!(incoming_conn.public_key, public_key);
        match incoming_conn.inner {
            IncomingConnInner::Listen(_incoming_listen) => {}
            _ => panic!("Wrong IncomingConnInner"),
        };

        let (sender, receiver) = mpsc::channel::<Vec<u8>>(0);
        let accept_public_key = PublicKey::from(&[0x22; PUBLIC_KEY_LEN]);
        let first_msg = InitConnection::Accept(accept_public_key.clone());
        let ser_first_msg = first_msg.proto_serialize();
        let public_key = PublicKey::from(&[0x77; PUBLIC_KEY_LEN]);
        let keepalive_transform = FuncFutTransform::new(|x| Box::pin(future::ready(x)));
        let incoming_conn = await!(dispatch_conn(
            sender,
            receiver,
            public_key.clone(),
            ser_first_msg,
            keepalive_transform
        ))
        .unwrap();

        assert_eq!(incoming_conn.public_key, public_key);
        match incoming_conn.inner {
            IncomingConnInner::Accept(incoming_accept) => {
                assert_eq!(incoming_accept.accept_public_key, accept_public_key)
            }
            _ => panic!("Wrong IncomingConnInner"),
        };

        let (sender, receiver) = mpsc::channel::<Vec<u8>>(0);
        let connect_public_key = PublicKey::from(&[0x33; PUBLIC_KEY_LEN]);
        let first_msg = InitConnection::Connect(connect_public_key.clone());
        let ser_first_msg = first_msg.proto_serialize();
        let public_key = PublicKey::from(&[0x77; PUBLIC_KEY_LEN]);
        let keepalive_transform = FuncFutTransform::new(|x| Box::pin(future::ready(x)));
        let incoming_conn = await!(dispatch_conn(
            sender,
            receiver,
            public_key.clone(),
            ser_first_msg,
            keepalive_transform
        ))
        .unwrap();

        assert_eq!(incoming_conn.public_key, public_key);
        match incoming_conn.inner {
            IncomingConnInner::Connect(incoming_connect) => {
                assert_eq!(incoming_connect.connect_public_key, connect_public_key)
            }
            _ => panic!("Wrong IncomingConnInner"),
        };
    }

    #[test]
    fn test_dispatch_conn_basic() {
        let mut thread_pool = ThreadPool::new().unwrap();
        thread_pool.run(task_dispatch_conn_basic(thread_pool.clone()));
    }

    async fn task_dispatch_conn_invalid_first_msg(spawner: impl Spawn + Clone) {
        // Create a mock time service:
        let (_tick_sender, tick_receiver) = mpsc::channel::<()>(0);
        let _timer_client = create_timer_incoming(tick_receiver, spawner.clone()).unwrap();

        let (sender, receiver) = mpsc::channel::<Vec<u8>>(0);
        let ser_first_msg = b"This is an invalid message".to_vec();
        let public_key = PublicKey::from(&[0x77; PUBLIC_KEY_LEN]);
        let keepalive_transform = FuncFutTransform::new(|x| Box::pin(future::ready(x)));
        let res = await!(dispatch_conn(
            sender,
            receiver,
            public_key.clone(),
            ser_first_msg,
            keepalive_transform
        ));
        assert!(res.is_none());
    }

    #[test]
    fn test_dispatch_conn_invalid_first_msg() {
        let mut thread_pool = ThreadPool::new().unwrap();
        thread_pool.run(task_dispatch_conn_invalid_first_msg(thread_pool.clone()));
    }

    #[test]
    fn test_conn_processor_basic() {
        let mut thread_pool = ThreadPool::new().unwrap();

        // Create a mock time service:
        let (_tick_sender, tick_receiver) = mpsc::channel::<()>(0);
        let timer_client = create_timer_incoming(tick_receiver, thread_pool.clone()).unwrap();

        let public_key = PublicKey::from(&[0x77; PUBLIC_KEY_LEN]);
        let (local_sender, _remote_receiver) = mpsc::channel::<Vec<u8>>(0);
        let (mut remote_sender, local_receiver) = mpsc::channel::<Vec<u8>>(0);

        let incoming_conns =
            stream::iter::<_>(vec![(public_key.clone(), (local_sender, local_receiver))]);

        let conn_timeout_ticks = 16;
        let keepalive_transform = FuncFutTransform::new(|x| Box::pin(future::ready(x)));

        let processed_conns = conn_processor(
            incoming_conns,
            keepalive_transform,
            timer_client,
            conn_timeout_ticks,
        );

        let processed_conns = Box::pin(processed_conns);

        let first_msg = InitConnection::Listen;
        let ser_first_msg = first_msg.proto_serialize();
        thread_pool
            .spawn(async move {
                await!(remote_sender.send(ser_first_msg).map(|res| {
                    match res {
                        Ok(_remote_sender) => (),
                        Err(_) => unreachable!("Sending first message failed!"),
                    }
                }))
            })
            .unwrap();

        let (conn, processed_conns) = thread_pool.run(receive(processed_conns)).unwrap();
        assert_eq!(conn.public_key, public_key);
        match conn.inner {
            IncomingConnInner::Listen(_incoming_listen) => {}
            _ => panic!("Incorrect processed conn"),
        };

        assert!(thread_pool.run(receive(processed_conns)).is_none());
    }
}
