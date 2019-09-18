use bytes::Bytes;

use futures::channel::mpsc;
use futures::compat::{Compat, Future01CompatExt};
use futures::task::{Spawn, SpawnExt};
use futures::{FutureExt, SinkExt, StreamExt};

use futures_01::sink::Sink as Sink01;
use futures_01::stream::Stream as Stream01;

use tokio::codec::{Framed, LengthDelimitedCodec};
use tokio::net::TcpStream;

use common::conn::ConnPairVec;

/// Convert a connection pair (sender Sink, receiver Stream) of Futures 0.1
/// to a pair of (mpsc::Sender, mpsc::Receiver) of Futures 0.3.
fn conn_pair_01_to_03<T, ST, SI, S>(
    conn_pair_01: (SI, ST),
    spawner: &mut S,
) -> (mpsc::Sender<T>, mpsc::Receiver<T>)
where
    T: Send + 'static,
    ST: Stream01<Item = T> + Send + 'static,
    SI: Sink01<SinkItem = T> + Send + 'static,
    S: Spawn + Send,
{
    let (sender_01, receiver_01) = conn_pair_01;

    let (mut user_sender_03, from_user_sender_03) = mpsc::channel::<Result<T, ()>>(0);
    let (to_user_receiver_03, mut user_receiver_03) = mpsc::channel::<Result<T, ()>>(0);

    // Forward messages from user_sender:
    let from_user_sender_01 = Compat::new(from_user_sender_03).map_err(|_| ());

    let sender_01 = sender_01
        .sink_map_err(|_| ())
        .with(|t: T| -> Result<T, ()> { Ok(t) });

    let send_forward_03 = sender_01.send_all(from_user_sender_01).compat().map(|_| ());

    drop(spawner.spawn(send_forward_03));

    // Forward messages to user_receiver:
    let to_user_receiver_01 = to_user_receiver_03
        .compat()
        .sink_map_err(|_| ())
        .with(|t: T| -> Result<Result<T, ()>, ()> { Ok(Ok(t)) });

    let receiver_01 = receiver_01.map_err(|_| ());

    let recv_forward_01 = to_user_receiver_01
        .send_all(receiver_01)
        .compat()
        .map(|_| ());

    // We keep a handle, to be able to cancel the recv task later:
    let opt_recv_handle = spawner.spawn_with_handle(recv_forward_01);

    // We want to give the user sender and receiver of T (And not Result<T,()>),
    // so another adapting layer is required:

    let (user_sender, mut from_user_sender) = mpsc::channel::<T>(0);
    let (mut to_user_receiver, user_receiver) = mpsc::channel::<T>(0);

    // Forward user_receiver:
    let opt_user_receiver = spawner.spawn_with_handle(async move {
        while let Some(Ok(data)) = user_receiver_03.next().await {
            if to_user_receiver.send(data).await.is_err() {
                warn!("conn_pair_01_to_03(): to_user_receiver.send() error");
                return;
            }
        }
    });

    // Forward user_sender:
    let _ = spawner.spawn(async move {
        while let Some(data) = from_user_sender.next().await {
            if user_sender_03.send(Ok(data)).await.is_err() {
                warn!("Forward user_sender error");
                break;
            }
        }
        // The user closed the sender. We close the connection aggressively.
        // We have to drop all the receiver tasks, because closing the sender is not enough for
        // closing the connection.
        //
        // See also:
        // https://users.rust-lang.org/t/
        //      tokio-tcp-connection-not-closed-when-sender-is-dropped-futures-0-3-compat-layer/26910/4
        drop(opt_recv_handle);
        drop(opt_user_receiver);
    });

    (user_sender, user_receiver)
}

pub fn tcp_stream_to_conn_pair<S>(
    tcp_stream: TcpStream,
    max_frame_length: usize,
    spawner: &mut S,
) -> ConnPairVec
where
    S: Spawn + Send,
{
    let mut codec = LengthDelimitedCodec::new();
    codec.set_max_frame_length(max_frame_length);
    let (sender_01, receiver_01) = Framed::new(tcp_stream, codec).split();

    // Conversion layer between Vec<u8> to Bytes:
    let sender_01 = sender_01
        .sink_map_err(|_| ())
        .with(|vec: Vec<u8>| -> Result<Bytes, ()> { Ok(Bytes::from(vec)) });

    let receiver_01 = receiver_01.map(|bytes| bytes.to_vec());

    conn_pair_01_to_03((sender_01, receiver_01), spawner)
}

#[cfg(test)]
mod tests {
    use super::*;

    use futures::executor::ThreadPool;
    use futures_01::sync::mpsc as mpsc01;

    async fn task_conn_pair_01_to_03_basic<S>(mut spawner: S)
    where
        S: Spawn + Send,
    {
        let (sender_01, receiver_01) = mpsc01::channel::<u32>(0);
        let (mut sender_03, mut receiver_03) =
            conn_pair_01_to_03((sender_01, receiver_01), &mut spawner);
        sender_03.send(0x1337u32).await.unwrap();
        assert_eq!(receiver_03.next().await, Some(0x1337u32));

        drop(sender_03);
        assert!(receiver_03.next().await.is_none());
    }

    #[test]
    fn test_conn_pair_01_to_03_basic() {
        let mut thread_pool = ThreadPool::new().unwrap();
        thread_pool.run(task_conn_pair_01_to_03_basic(thread_pool.clone()));
    }

    async fn task_conn_pair_01_to_03_receiver_dropped<S>(mut spawner: S)
    where
        S: Spawn + Send,
    {
        let (sender_01, receiver_01) = mpsc01::channel::<u32>(0);
        let (mut sender_03, receiver_03) =
            conn_pair_01_to_03((sender_01, receiver_01), &mut spawner);

        drop(receiver_03);

        // The sender should be closed after a while.
        // We are using many intermediate buffers, so the closing
        // will only be detected after attempting to send a few messages.
        let mut sender_closed = false;
        for _ in 0..20 {
            if let Err(_) = sender_03.send(0u32).await {
                sender_closed = true;
            }
        }
        assert!(sender_closed);
    }

    #[test]
    fn test_conn_pair_01_to_03_receiver_dropped() {
        let mut thread_pool = ThreadPool::new().unwrap();
        thread_pool.run(task_conn_pair_01_to_03_receiver_dropped(
            thread_pool.clone(),
        ));
    }
}
