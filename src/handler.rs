use crate::protocol::{PushInStreamSink, PushOutStreamSink, PushProtocol};
use crate::{PushError, PushEvent};
use core::pin::Pin;
use futures::prelude::*;
use futures_util::task::{Context, Poll};
use libp2p::{
    core::{InboundUpgrade, OutboundUpgrade},
    swarm::{
        KeepAlive, NegotiatedSubstream, ProtocolsHandler, ProtocolsHandlerEvent,
        ProtocolsHandlerUpgrErr, SubstreamProtocol,
    },
};
use log::debug;
use std::collections::VecDeque;

#[derive(Debug)]
enum OutboundSinkState {
    Empty,
    PendingCreate,
    Init,
    PendingFlush,
}

pub struct OneDirectionPushHandler {
    pub is_server: bool,
    pub pending_events: VecDeque<PushEvent>,
    send_event: Option<PushEvent>,
    in_substreams: Vec<PushInStreamSink<NegotiatedSubstream>>,
    out_substream: Option<PushOutStreamSink<NegotiatedSubstream>>,
    out_substream_state: OutboundSinkState,
}

impl OneDirectionPushHandler {
    pub fn new(is_server: bool) -> Self {
        OneDirectionPushHandler {
            is_server,
            pending_events: VecDeque::new(),
            send_event: None,
            in_substreams: vec![],
            out_substream: None,
            out_substream_state: OutboundSinkState::Empty,
        }
    }
}

impl ProtocolsHandler for OneDirectionPushHandler {
    type InEvent = PushEvent;
    type OutEvent = PushEvent;
    type Error = PushError;
    type InboundProtocol = PushProtocol;
    type OutboundProtocol = PushProtocol;
    type OutboundOpenInfo = ();

    fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol> {
        debug!("==== push handler listen_protocol");
        SubstreamProtocol::new(PushProtocol)
    }

    fn inject_fully_negotiated_inbound(
        &mut self,
        protocol: <Self::InboundProtocol as InboundUpgrade<NegotiatedSubstream>>::Output,
    ) {
        debug!("==== negotiated inbound");
        self.in_substreams.push(protocol);
    }

    fn inject_fully_negotiated_outbound(
        &mut self,
        protocol: <Self::OutboundProtocol as OutboundUpgrade<NegotiatedSubstream>>::Output,
        _info: Self::OutboundOpenInfo,
    ) {
        debug!("==== negotiated outbound");
        if let OutboundSinkState::PendingCreate = self.out_substream_state {
            if self.out_substream.is_some() {
                panic!("duplicate outbound");
            }
            self.out_substream.replace(protocol);
        }
    }

    fn inject_event(&mut self, event: Self::InEvent) {
        debug!("=== push event handler inject_event: {:?}", event);
        self.pending_events.push_back(event);
    }

    fn inject_dial_upgrade_error(
        &mut self,
        _info: Self::OutboundOpenInfo,
        error: ProtocolsHandlerUpgrErr<std::io::Error>,
    ) {
        debug!(
            "==== push event handler inject dial upgrade error: {:?}",
            error
        );
    }

    fn connection_keep_alive(&self) -> KeepAlive {
        KeepAlive::Yes
    }

    fn poll(
        &mut self,
        cx: &mut Context,
    ) -> Poll<
        ProtocolsHandlerEvent<
            Self::OutboundProtocol,
            Self::OutboundOpenInfo,
            Self::OutEvent,
            Self::Error,
        >,
    > {
        debug!("==== push event handler polling");
        if self.is_server {
            for s in self.in_substreams.iter_mut() {
                match Stream::poll_next(Pin::new(s), cx) {
                    Poll::Ready(Some(res)) => match res {
                        Ok(e) => {
                            debug!("=== in comming event");
                            return Poll::Ready(ProtocolsHandlerEvent::Custom(e));
                        }
                        Err(e) => {
                            return Poll::Ready(ProtocolsHandlerEvent::Close(PushError::from(e)))
                        }
                    },
                    Poll::Ready(None) => {
                        return Poll::Ready(ProtocolsHandlerEvent::Close(PushError::SubStreamEof))
                    }
                    Poll::Pending => (),
                }
            }
        } else {
            if let Some(ref mut out_sink) = self.out_substream {
                debug!(
                    "=== has out substream, state: {:?}",
                    self.out_substream_state
                );
                loop {
                    match self.out_substream_state {
                        OutboundSinkState::Init => {
                            if self.send_event.is_none() {
                                self.send_event = self.pending_events.pop_front();
                                debug!("=== get pending event: {:?}", self.send_event);
                            }
                            if self.send_event.is_some() {
                                match Sink::poll_ready(Pin::new(out_sink), cx) {
                                    Poll::Ready(Ok(())) => {
                                        debug!("=== out sink ready");
                                        let event = self.send_event.take().unwrap();
                                        match Sink::start_send(Pin::new(out_sink), event) {
                                            _ => {
                                                self.out_substream_state =
                                                    OutboundSinkState::PendingFlush
                                            }
                                        }
                                    }
                                    Poll::Ready(Err(e)) => {
                                        return Poll::Ready(ProtocolsHandlerEvent::Close(
                                            PushError::from(e),
                                        ))
                                    }
                                    Poll::Pending => return Poll::Pending,
                                }
                            } else {
                                return Poll::Pending;
                            }
                        }
                        OutboundSinkState::PendingFlush => {
                            debug!("=== polling flush");
                            match Sink::poll_flush(Pin::new(out_sink), cx) {
                                Poll::Ready(Ok(())) => {
                                    debug!("=== send event flushed");
                                    self.out_substream_state = OutboundSinkState::Init;
                                    return Poll::Pending;
                                }
                                Poll::Ready(Err(e)) => {
                                    return Poll::Ready(ProtocolsHandlerEvent::Close(
                                        PushError::from(e),
                                    ))
                                }
                                Poll::Pending => return Poll::Pending,
                            }
                        }
                        _ => self.out_substream_state = OutboundSinkState::Init,
                    };
                }
            } else {
                // if there is no outbound substream, request one
                match self.out_substream_state {
                    OutboundSinkState::Empty => {
                        self.out_substream_state = OutboundSinkState::PendingCreate;
                        return Poll::Ready(ProtocolsHandlerEvent::OutboundSubstreamRequest {
                            protocol: SubstreamProtocol::new(PushProtocol),
                            info: (),
                        });
                    }
                    _ => (),
                }
            }
        }
        Poll::Pending
    }
}
