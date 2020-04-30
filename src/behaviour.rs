use futures_util::task::{Context, Poll};
use libp2p::{
    core::{connection::ConnectionId, Multiaddr},
    swarm::{
        DialPeerCondition, NetworkBehaviour, NetworkBehaviourAction, NotifyHandler, PollParameters,
    },
    PeerId,
};
use log::debug;
use std::collections::VecDeque;

use crate::handler::OneDirectionPushHandler;

#[derive(Debug, Clone)]
pub enum PushEvent {
    Message(Vec<u8>),
}

pub struct OneDirectionPush {
    is_server: bool,
    peer_id: Option<PeerId>,
    events: VecDeque<NetworkBehaviourAction<PushEvent, PushEvent>>,
}

impl OneDirectionPush {
    pub fn new(is_server: bool) -> Self {
        OneDirectionPush {
            is_server,
            peer_id: None,
            events: VecDeque::new(),
        }
    }

    pub fn push(&mut self, data: Vec<u8>) {
        if !self.is_server {
            if let Some(ref peer_id) = self.peer_id {
                self.events
                    .push_back(NetworkBehaviourAction::NotifyHandler {
                        peer_id: peer_id.clone(),
                        handler: NotifyHandler::Any,
                        event: PushEvent::Message(data),
                    })
            }
        }
    }
}

impl NetworkBehaviour for OneDirectionPush {
    type ProtocolsHandler = OneDirectionPushHandler;
    type OutEvent = PushEvent;

    fn new_handler(&mut self) -> Self::ProtocolsHandler {
        OneDirectionPushHandler::new(self.is_server)
    }

    fn addresses_of_peer(&mut self, _peer_id: &PeerId) -> Vec<Multiaddr> {
        Vec::new()
    }

    fn inject_connected(&mut self, peer_id: &PeerId) {
        println!(
            "==== push event behaviour inject connected to {:?}",
            peer_id
        );
        if !self.is_server {
            self.peer_id.replace(peer_id.clone());
        }
    }

    fn inject_disconnected(&mut self, peer_id: &PeerId) {
        if !self.is_server {
            debug!("=== push event inject disconnected: {:?}", peer_id);
            self.peer_id.take();
            self.events.push_back(NetworkBehaviourAction::DialPeer {
                peer_id: peer_id.clone(),
                condition: DialPeerCondition::NotDialing,
            })
        }
    }

    fn inject_event(&mut self, _peer_id: PeerId, _connection: ConnectionId, event: PushEvent) {
        debug!("=== push event behaviour inject_event: {:?}", event);
        self.events
            .push_front(NetworkBehaviourAction::GenerateEvent(event));
    }

    fn poll(
        &mut self,
        _: &mut Context,
        _: &mut impl PollParameters,
    ) -> Poll<NetworkBehaviourAction<PushEvent, PushEvent>> {
        if let Some(e) = self.events.pop_back() {
            return Poll::Ready(e);
        } else {
            Poll::Pending
        }
    }
}
