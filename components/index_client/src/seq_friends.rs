use futures::channel::{mpsc, oneshot};
use futures::task::{Spawn, SpawnError, SpawnExt};
use futures::{SinkExt, StreamExt};

use proto::crypto::PublicKey;
use proto::index_client::messages::{FriendInfo, IndexMutation, UpdateFriend};

use crate::seq_map::SeqMap;

pub type SeqFriends = SeqMap<PublicKey, FriendInfo>;

pub enum SeqFriendsRequest {
    Mutate(IndexMutation, oneshot::Sender<()>),
    ResetCountdown(oneshot::Sender<()>),
    NextUpdate(oneshot::Sender<Option<(usize, UpdateFriend)>>),
}

#[derive(Debug)]
pub enum SeqFriendsClientError {
    SendRequestError,
    RecvResponseError,
}

#[derive(Clone)]
pub struct SeqFriendsClient {
    requests_sender: mpsc::Sender<SeqFriendsRequest>,
}

fn apply_index_mutation(seq_friends: &mut SeqFriends, index_mutation: &IndexMutation) {
    match index_mutation {
        IndexMutation::UpdateFriend(update_friend) => {
            let friend_info = FriendInfo {
                send_capacity: update_friend.send_capacity,
                recv_capacity: update_friend.recv_capacity,
                rate: update_friend.rate.clone(),
            };
            let _ = seq_friends.update(update_friend.public_key.clone(), friend_info);
        }
        IndexMutation::RemoveFriend(public_key) => {
            let _ = seq_friends.remove(public_key);
        }
    }
}

async fn seq_friends_loop(
    mut seq_friends: SeqFriends,
    mut requests_receiver: mpsc::Receiver<SeqFriendsRequest>,
) {
    while let Some(request) = requests_receiver.next().await {
        match request {
            SeqFriendsRequest::Mutate(index_mutation, response_sender) => {
                apply_index_mutation(&mut seq_friends, &index_mutation);
                let _ = response_sender.send(());
            }
            SeqFriendsRequest::ResetCountdown(response_sender) => {
                seq_friends.reset_countdown();
                let _ = response_sender.send(());
            }
            SeqFriendsRequest::NextUpdate(response_sender) => {
                let update_friend =
                    seq_friends
                        .next()
                        .map(|(cycle_countdown, (public_key, friend_info))| {
                            let FriendInfo {
                                send_capacity,
                                recv_capacity,
                                rate,
                            } = friend_info;
                            let update_friend = UpdateFriend {
                                public_key,
                                send_capacity,
                                recv_capacity,
                                rate,
                            };
                            (cycle_countdown, update_friend)
                        });
                let _ = response_sender.send(update_friend);
            }
        }
    }
}

impl SeqFriendsClient {
    pub fn new(requests_sender: mpsc::Sender<SeqFriendsRequest>) -> Self {
        SeqFriendsClient { requests_sender }
    }

    pub async fn mutate(
        &mut self,
        index_mutation: IndexMutation,
    ) -> Result<(), SeqFriendsClientError> {
        let (sender, receiver) = oneshot::channel();
        let request = SeqFriendsRequest::Mutate(index_mutation, sender);
        self.requests_sender.send(request).await
            .map_err(|_| SeqFriendsClientError::SendRequestError)?;
        Ok(receiver.await.map_err(|_| SeqFriendsClientError::RecvResponseError)?)
    }

    pub async fn reset_countdown(&mut self) -> Result<(), SeqFriendsClientError> {
        let (sender, receiver) = oneshot::channel();
        let request = SeqFriendsRequest::ResetCountdown(sender);
        self.requests_sender.send(request)
            .await
            .map_err(|_| SeqFriendsClientError::SendRequestError)?;
        Ok(receiver.await.map_err(|_| SeqFriendsClientError::RecvResponseError)?)
    }

    pub async fn next_update(
        &mut self,
    ) -> Result<Option<(usize, UpdateFriend)>, SeqFriendsClientError> {
        let (sender, receiver) = oneshot::channel();
        let request = SeqFriendsRequest::NextUpdate(sender);
        self.requests_sender.send(request)
            .await
            .map_err(|_| SeqFriendsClientError::SendRequestError)?;
        Ok(receiver.await.map_err(|_| SeqFriendsClientError::RecvResponseError)?)
    }
}

pub fn create_seq_friends_service<S>(
    seq_friends: SeqFriends,
    mut spawner: S,
) -> Result<SeqFriendsClient, SpawnError>
where
    S: Spawn,
{
    let (requests_sender, requests_receiver) = mpsc::channel(0);
    let loop_fut = seq_friends_loop(seq_friends, requests_receiver);
    spawner.spawn(loop_fut)?;

    Ok(SeqFriendsClient::new(requests_sender))
}
