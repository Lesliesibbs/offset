use std::collections::HashMap;
use std::hash::Hash;

use serde::{Deserialize, Serialize};

use common::ser_string::{from_base64, from_string, to_base64, to_string};

use app::common::{NetAddress, PrivateKey, PublicKey, Uid};
use app::conn::AppPermissions;

use crate::compact_node::{CompactReport, CompactToUser, UserToCompact};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
pub struct NodeName(String);

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
pub struct NodeId(#[serde(serialize_with = "to_string", deserialize_with = "from_string")] pub u64);

impl NodeName {
    #[allow(unused)]
    pub fn new(node_name: String) -> Self {
        Self(node_name)
    }
    #[allow(unused)]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeInfoLocal {
    #[serde(serialize_with = "to_base64", deserialize_with = "from_base64")]
    pub node_public_key: PublicKey,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeInfoRemote {
    #[serde(serialize_with = "to_base64", deserialize_with = "from_base64")]
    pub app_public_key: PublicKey,
    #[serde(serialize_with = "to_base64", deserialize_with = "from_base64")]
    pub node_public_key: PublicKey,
    pub node_address: NetAddress,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeInfo {
    Local(NodeInfoLocal),
    Remote(NodeInfoRemote),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeStatus {
    pub is_open: bool,
    pub info: NodeInfo,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateNodeLocal {
    pub node_name: NodeName,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateNodeRemote {
    pub node_name: NodeName,
    #[serde(serialize_with = "to_base64", deserialize_with = "from_base64")]
    pub app_private_key: PrivateKey,
    #[serde(serialize_with = "to_base64", deserialize_with = "from_base64")]
    pub node_public_key: PublicKey,
    pub node_address: NetAddress,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ResponseOpenNode {
    Success(NodeName, NodeId, AppPermissions, CompactReport), // (node_name, node_id, compact_report)
    Failure(NodeName),
}

pub type NodesInfo = HashMap<NodeName, NodeInfo>;

pub type NodesStatus = HashMap<NodeName, NodeStatus>;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum RequestCreateNode {
    CreateNodeLocal(CreateNodeLocal),
    CreateNodeRemote(CreateNodeRemote),
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ServerToUser {
    ResponseOpenNode(ResponseOpenNode),
    /// A map of all nodes and their current status
    NodesStatus(NodesStatus),
    /// A message received from a specific node
    Node(NodeId, CompactToUser), // (node_id, compact_to_user)
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ServerToUserAck {
    ServerToUser(ServerToUser),
    #[serde(serialize_with = "to_base64", deserialize_with = "from_base64")]
    Ack(Uid),
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum UserToServer {
    RequestCreateNode(RequestCreateNode),
    RequestRemoveNode(NodeName),
    RequestOpenNode(NodeName),
    RequestCloseNode(NodeId), // node_id
    /// A message sent to a specific node
    Node(NodeId, UserToCompact), // (node_id, user_to_compact)
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserToServerAck {
    #[serde(serialize_with = "to_base64", deserialize_with = "from_base64")]
    pub request_id: Uid,
    pub inner: UserToServer,
}
