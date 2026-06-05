use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use clap::Parser;
use reqwest::Client;
use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Instant};
use tokio::{net::UdpSocket, sync::RwLock};

#[derive(Parser)]
#[command(name = "rdoh", about = "DNS-over-HTTPS proxy")]
struct Args {
    /// Listen address
    #[arg(short, long, default_value = "0.0.0.0:53")]
    listen: SocketAddr,
    /// DoH upstream URL
    #[arg(short, long, default_value = "https://8.8.8.8/dns-query")]
    upstream: String,
}

struct CacheEntry {
    response: Vec<u8>,
    expires: Instant,
}

type Cache = Arc<RwLock<HashMap<Vec<u8>, CacheEntry>>>;

fn extract_min_ttl(data: &[u8]) -> u32 {
    let mut pos = 12usize;
    let qdcount = u16::from_be_bytes([data[4], data[5]]) as usize;
    // Skip questions
    for _ in 0..qdcount {
        while pos < data.len() && data[pos] != 0 {
            pos += data[pos] as usize + 1;
        }
        pos += 5; // null byte + qtype(2) + qclass(2)
    }
    let ancount = u16::from_be_bytes([data[6], data[7]]) as usize;
    let mut min_ttl = 300u32;
    for _ in 0..ancount {
        if pos + 2 > data.len() { break; }
        // Skip name (pointer or labels)
        if data[pos] & 0xc0 == 0xc0 {
            pos += 2;
        } else {
            while pos < data.len() && data[pos] != 0 {
                pos += data[pos] as usize + 1;
            }
            pos += 1;
        }
        if pos + 10 > data.len() { break; }
        pos += 4; // type + class
        let ttl = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        min_ttl = min_ttl.min(ttl);
        let rdlen = u16::from_be_bytes([data[pos + 4], data[pos + 5]]) as usize;
        pos += 6 + rdlen;
    }
    min_ttl.max(1)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let sock = Arc::new(UdpSocket::bind(args.listen).await?);
    let client = Client::builder()
        .use_rustls_tls()
        .build()?;
    let cache: Cache = Arc::new(RwLock::new(HashMap::new()));
    let upstream = Arc::new(args.upstream);

    eprintln!("pahudoh listening on {} → {}", args.listen, upstream);

    let mut buf = [0u8; 512];
    loop {
        let (len, addr) = sock.recv_from(&mut buf).await?;
        let data = buf[..len].to_vec();
        let sock = sock.clone();
        let client = client.clone();
        let cache = cache.clone();
        let upstream = upstream.clone();

        tokio::spawn(async move {
            if let Err(e) = handle(&data, addr, &sock, &client, &cache, &upstream).await {
                eprintln!("ERR {addr}: {e}");
            }
        });
    }
}

async fn handle(
    data: &[u8],
    addr: SocketAddr,
    sock: &UdpSocket,
    client: &Client,
    cache: &Cache,
    upstream: &str,
) -> anyhow::Result<()> {
    let key = data[2..].to_vec(); // skip txn ID for cache key

    // Check cache
    {
        let c = cache.read().await;
        if let Some(entry) = c.get(&key) {
            if Instant::now() < entry.expires {
                let mut resp = entry.response.clone();
                resp[0] = data[0]; // patch txn ID
                resp[1] = data[1];
                sock.send_to(&resp, addr).await?;
                return Ok(());
            }
        }
    }

    // Forward via DoH
    let b64 = URL_SAFE_NO_PAD.encode(data);
    let url = format!("{upstream}?dns={b64}");
    let resp = client
        .get(&url)
        .header("Accept", "application/dns-message")
        .header("Host", "dns.google")
        .send()
        .await?
        .bytes()
        .await?;

    let ttl = extract_min_ttl(&resp);
    let expires = Instant::now() + std::time::Duration::from_secs(ttl as u64);

    // Store in cache
    {
        let mut c = cache.write().await;
        c.insert(key, CacheEntry { response: resp.to_vec(), expires });
    }

    sock.send_to(&resp, addr).await?;
    Ok(())
}
