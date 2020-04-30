use libp2p::{identity, PeerId, Transport, tcp, secio, yamux, mplex, core};

use std::{error, io, time::Duration};

pub fn build_tcp_transport(keypair: identity::Keypair)
    -> io::Result<impl Transport<Output = (PeerId, impl core::muxing::StreamMuxer<OutboundSubstream = impl Send, Substream = impl Send, Error = impl Into<io::Error>> + Send + Sync), Error = impl error::Error + Send, Listener = impl Send, Dial = impl Send, ListenerUpgrade = impl Send> + Clone>
{
    let transport = {
        tcp::TokioTcpConfig::new().nodelay(true)
    };

    Ok(transport
        .upgrade(core::upgrade::Version::V1)
        .authenticate(secio::SecioConfig::new(keypair))
        .multiplex(core::upgrade::SelectUpgrade::new(yamux::Config::default(), mplex::MplexConfig::new()))
        .map(|(peer, muxer), _| (peer, core::muxing::StreamMuxerBox::new(muxer)))
        .timeout(Duration::from_secs(20)))
}