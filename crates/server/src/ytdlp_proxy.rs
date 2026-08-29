use crate::network_policy::{NetworkAccessScope, NetworkPolicy};
use std::net::SocketAddr;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::{Duration, timeout},
};
use tracing::{debug, warn};
use url::Url;

const MAX_PROXY_HEADER_BYTES: usize = 8 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
pub struct YoutubePolicyProxy {
    url: String,
}

impl YoutubePolicyProxy {
    pub async fn start(policy: NetworkPolicy) -> Result<Self, String> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(|error| format!("download policy proxy could not bind: {error}"))?;
        let address = listener
            .local_addr()
            .map_err(|error| format!("download policy proxy address is unavailable: {error}"))?;
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, peer)) if peer.ip().is_loopback() => {
                        let request_policy = policy.clone();
                        tokio::spawn(async move {
                            if let Err(error) = handle_connection(stream, request_policy).await {
                                debug!(%error, "download policy proxy rejected a request");
                            }
                        });
                    }
                    Ok((_stream, peer)) => {
                        warn!(%peer, "download policy proxy rejected a non-loopback peer");
                    }
                    Err(error) => {
                        warn!(%error, "download policy proxy accept failed");
                    }
                }
            }
        });
        Ok(Self {
            url: format!("http://{address}"),
        })
    }

    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }
}

async fn handle_connection(mut client: TcpStream, policy: NetworkPolicy) -> Result<(), String> {
    let request = read_proxy_header(&mut client).await?;
    let request = std::str::from_utf8(&request)
        .map_err(|_| "proxy request header is not valid UTF-8".to_owned())?;
    if request.lines().skip(1).any(|line| {
        line.split_once(':')
            .is_some_and(|(name, _)| name.eq_ignore_ascii_case("proxy-authorization"))
    }) {
        send_error(&mut client, 403, "Forbidden").await;
        return Err("proxy authentication is not supported".to_owned());
    }
    let first = request
        .lines()
        .next()
        .ok_or_else(|| "proxy request line is missing".to_owned())?;
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let authority = parts.next().unwrap_or_default();
    let version = parts.next().unwrap_or_default();
    if method != "CONNECT" || authority.is_empty() || !version.starts_with("HTTP/") {
        send_error(&mut client, 405, "Method Not Allowed").await;
        return Err("only HTTP CONNECT tunnelling is supported".to_owned());
    }
    let origin = connect_origin(authority)?;
    let validated = match policy
        .validate(origin.as_str(), NetworkAccessScope::Youtube)
        .await
    {
        Ok(validated) => validated,
        Err(error) => {
            send_error(&mut client, 403, "Forbidden").await;
            return Err(error);
        }
    };
    let upstream = connect_pinned(validated.addresses()).await;
    let mut upstream = match upstream {
        Ok(stream) => stream,
        Err(error) => {
            send_error(&mut client, 502, "Bad Gateway").await;
            return Err(error);
        }
    };
    client
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        .await
        .map_err(|_| "proxy response could not be written".to_owned())?;
    tokio::io::copy_bidirectional(&mut client, &mut upstream)
        .await
        .map_err(|_| "proxy tunnel closed unexpectedly".to_owned())?;
    Ok(())
}

async fn read_proxy_header(stream: &mut TcpStream) -> Result<Vec<u8>, String> {
    let mut request = Vec::with_capacity(1024);
    let mut chunk = [0_u8; 1024];
    loop {
        let count = timeout(CONNECT_TIMEOUT, stream.read(&mut chunk))
            .await
            .map_err(|_| "proxy request timed out".to_owned())?
            .map_err(|_| "proxy request could not be read".to_owned())?;
        if count == 0 {
            return Err("proxy client disconnected before sending a request".to_owned());
        }
        request.extend_from_slice(&chunk[..count]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            return Ok(request);
        }
        if request.len() > MAX_PROXY_HEADER_BYTES {
            send_error(stream, 431, "Request Header Fields Too Large").await;
            return Err("proxy request header is too large".to_owned());
        }
    }
}

fn connect_origin(authority: &str) -> Result<Url, String> {
    let url = Url::parse(&format!("https://{authority}/"))
        .map_err(|_| "proxy CONNECT authority is invalid".to_owned())?;
    if url.host_str().is_none()
        || url.port_or_known_default().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("proxy CONNECT authority is invalid".to_owned());
    }
    Ok(url)
}

async fn connect_pinned(addresses: &[SocketAddr]) -> Result<TcpStream, String> {
    for address in addresses {
        if let Ok(Ok(stream)) = timeout(CONNECT_TIMEOUT, TcpStream::connect(address)).await {
            return Ok(stream);
        }
    }
    Err("validated destination could not be reached".to_owned())
}

async fn send_error(stream: &mut TcpStream, status: u16, reason: &str) {
    let response =
        format!("HTTP/1.1 {status} {reason}\r\nConnection: close\r\nContent-Length: 0\r\n\r\n");
    let _ = stream.write_all(response.as_bytes()).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_authorities_become_https_origins() {
        let standard = connect_origin("r3---sn.example.googlevideo.com:443").unwrap();
        assert_eq!(standard.scheme(), "https");
        assert_eq!(standard.host_str(), Some("r3---sn.example.googlevideo.com"));
        assert_eq!(standard.port_or_known_default(), Some(443));

        let ipv6 = connect_origin("[2606:4700:4700::1111]:443").unwrap();
        assert_eq!(ipv6.port_or_known_default(), Some(443));
    }

    #[test]
    fn connect_authorities_reject_credentials_and_missing_hosts() {
        assert!(connect_origin("user@example.com:443").is_err());
        assert!(connect_origin(":443").is_err());
    }
}
