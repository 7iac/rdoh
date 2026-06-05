# rdoh

A minimal DNS-over-HTTPS proxy in Rust. Single binary, in-memory TTL cache, zero config.

## Why

Your ISP blocks or throttles UDP port 53 to external resolvers. rdoh listens on port 53 locally and forwards all queries over HTTPS (port 443), which can't be easily blocked.

```
Client → UDP:53 → rdoh → HTTPS:443 → dns.google → response
                    ↕
              TTL cache (0ms on hit)
```

## Install

```bash
cargo install --path .
```

Or download a prebuilt binary from [Releases](https://github.com/7iac/rdoh/releases).

## Usage

```bash
# Default: listen 0.0.0.0:53, upstream https://8.8.8.8/dns-query
sudo rdoh

# Custom listen address and upstream
rdoh -l 127.0.0.1:5053 -u https://1.1.1.1/dns-query
```

## Performance

| | Cold | Cached |
|---|---|---|
| Query time | ~40ms | 0ms |

Binary size: **2.8MB** (macOS arm64, stripped + LTO)

## Systemd

```ini
[Unit]
Description=rdoh DNS-over-HTTPS proxy
After=network-online.target

[Service]
ExecStart=/usr/local/bin/rdoh
Restart=always

[Install]
WantedBy=multi-user.target
```

## macOS launchd

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>com.rdoh.proxy</string>
    <key>ProgramArguments</key>
    <array><string>/usr/local/bin/rdoh</string></array>
    <key>RunAtLoad</key><true/>
    <key>KeepAlive</key><true/>
</dict>
</plist>
```

## License

MIT
