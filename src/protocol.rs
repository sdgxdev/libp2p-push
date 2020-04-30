use crate::PushEvent;
use bytes::BytesMut;
use futures::prelude::*;
use futures_codec::Framed;
use libp2p::core::{InboundUpgrade, OutboundUpgrade, UpgradeInfo};
use log::debug;
use std::{io, iter};
use unsigned_varint::codec::UviBytes;

/// max push message length
const MAX_MSG_LEN: usize = 64 * 1024 * 1024;

#[derive(Default, Debug, Copy, Clone)]
pub struct PushProtocol;

impl UpgradeInfo for PushProtocol {
    type Info = &'static [u8];
    type InfoIter = iter::Once<Self::Info>;

    fn protocol_info(&self) -> Self::InfoIter {
        iter::once(b"/push/1.0.0")
    }
}

impl<TSocket> InboundUpgrade<TSocket> for PushProtocol
where
    TSocket: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    type Output = PushInStreamSink<TSocket>;
    type Error = io::Error;
    type Future = future::Ready<Result<Self::Output, io::Error>>;

    fn upgrade_inbound(self, socket: TSocket, _: Self::Info) -> Self::Future {
        let mut codec = UviBytes::default();
        codec.set_max_len(MAX_MSG_LEN);
        debug!("==== push event upgrade inbound");
        #[allow(unreachable_patterns)]
        future::ok(
            Framed::new(socket, codec)
                .err_into()
                .with::<_, _, fn(_) -> _, _>(|response| match response {
                    PushEvent::Message(msg) => future::ready(Ok(io::Cursor::new(msg))),
                    _ => unreachable!(),
                })
                .and_then::<_, fn(_) -> _>(|bytes| {
                    let request = PushEvent::Message(bytes.to_vec());
                    future::ready(Ok(request))
                }),
        )
    }
}

impl<TSocket> OutboundUpgrade<TSocket> for PushProtocol
where
    TSocket: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    type Output = PushOutStreamSink<TSocket>;
    type Error = io::Error;
    type Future = future::Ready<Result<Self::Output, io::Error>>;

    fn upgrade_outbound(self, socket: TSocket, _: Self::Info) -> Self::Future {
        let mut codec = UviBytes::default();
        codec.set_max_len(MAX_MSG_LEN);

        debug!("==== push event upgrade outbound");
        #[allow(unreachable_patterns)]
        future::ok(
            Framed::new(socket, codec)
                .err_into()
                .with::<_, _, fn(_) -> _, _>(|response| match response {
                    PushEvent::Message(msg) => future::ready(Ok(io::Cursor::new(msg))),
                    _ => unreachable!(),
                })
                .and_then::<_, fn(_) -> _>(|bytes| {
                    let request = PushEvent::Message(bytes.to_vec());
                    future::ready(Ok(request))
                }),
        )
    }
}

/// Sink of responses and stream of requests.
pub type PushInStreamSink<S> = PushStreamSink<S, PushEvent, PushEvent>;

/// Sink of requests and stream of responses.
pub type PushOutStreamSink<S> = PushStreamSink<S, PushEvent, PushEvent>;

pub type PushStreamSink<S, A, B> = stream::AndThen<
    sink::With<
        stream::ErrInto<Framed<S, UviBytes<io::Cursor<Vec<u8>>>>, io::Error>,
        io::Cursor<Vec<u8>>,
        A,
        future::Ready<Result<io::Cursor<Vec<u8>>, io::Error>>,
        fn(A) -> future::Ready<Result<io::Cursor<Vec<u8>>, io::Error>>,
    >,
    future::Ready<Result<B, io::Error>>,
    fn(BytesMut) -> future::Ready<Result<B, io::Error>>,
>;
