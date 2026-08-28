//! Networking helpers.

use socket2::{Domain, Protocol, Socket, Type};
use std::net::{Ipv6Addr, SocketAddr};
use tokio::net::TcpListener;

/// Bind a dual-stack (`::` with `IPV6_V6ONLY=false`) TCP listener on `port`, so a
/// single socket serves both IPv4 and IPv6 clients. Falls back to `0.0.0.0` if
/// the platform refuses dual-stack.
pub fn bind_dual_stack(port: u16) -> std::io::Result<std::net::TcpListener> {
    let addr: SocketAddr = (Ipv6Addr::UNSPECIFIED, port).into();
    let sock = Socket::new(Domain::IPV6, Type::STREAM, Some(Protocol::TCP))?;
    sock.set_only_v6(false).ok();
    sock.set_reuse_address(true).ok();
    if sock.bind(&addr.into()).is_err() {
        let v4: SocketAddr = (std::net::Ipv4Addr::UNSPECIFIED, port).into();
        return std::net::TcpListener::bind(v4);
    }
    sock.listen(1024)?;
    sock.set_nonblocking(true)?;
    Ok(sock.into())
}

/// Async version returning a tokio listener.
pub async fn listen(port: u16) -> std::io::Result<TcpListener> {
    TcpListener::from_std(bind_dual_stack(port)?)
}
