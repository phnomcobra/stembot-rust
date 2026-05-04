# stembot-rust

## Overview

StemBot is a distributed bot framework for deploying networks of lightweight agents that communicate over HTTP. Each agent runs as an Actix-Web server and can connect to peers, route messages across multi-hop networks, execute remote commands, and transfer files.

This is the Rust implementation of StemBot. It is protocol-compatible with [stembot-python](../stembot-python) — agents from both implementations can peer with and test one another interchangeably.

Key features:
- **Multi-agent networking** — agents discover peers and automatically share routing tables so messages can traverse multi-hop paths
- **Asynchronous tickets** — control requests are wrapped in tickets and delivered asynchronously, with optional path tracing through the network
- **AES-256 encryption** — every request and response is encrypted end-to-end using AES-256 in EAX mode
- **Polling mode** — agents with one-way connectivity can poll their peers rather than relying on inbound connections
- **CLI tools** — `agt-configure` for offline setup and `agt-control` for live agent management
- **In-memory collections** — messages, tickets, traces, and routes are held in named in-memory SQLite databases (via `file:?mode=memory` URIs) for low-latency access; peers and the key-value store are file-backed
- **Debian package** — `scripts/build_deb.sh` produces a `.deb` that installs binaries and a systemd service unit
- **Multi-stage Docker build** — the Dockerfile compiles with `rust:slim`, then copies the release binaries into a minimal `ubuntu:24.04` runtime image that includes logrotate for log management

## Installation

### From a Debian Package

```bash
# Build the .deb (requires Rust toolchain on the build host)
./scripts/build_deb.sh

# Install
sudo dpkg -i dist/stembot-rust_1.0.0_amd64.deb
```

The postinstall script creates `/var/agt/`, enables, and starts the `agt-server` systemd service.

### From Source

```bash
# Install Rust via rustup if not already present
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build release binaries
cargo build --release

# Binaries are placed in target/release/
```

### Configuring the Agent

There are two ways to configure an agent before starting it.

**Option 1: Environment variables**

```bash
export AGT_UUID="my-agent"
export AGT_PORT="8080"
export AGT_HOST="0.0.0.0"
export AGT_SECRET="mypassword"
export AGT_CLIENT_CONTROL_URL="http://127.0.0.1:8080/control"
export AGT_WORKERS="4"
export AGT_LOG_LEVEL_APP="info"
export AGT_LOG_LEVEL_API="error"
export AGT_PEER_TIMEOUT_SECS="60"
export AGT_PEER_REFRESH_SECS="30"
export AGT_MAX_WEIGHT="600"
export AGT_TICKET_TIMEOUT_SECS="600"
export AGT_MESSAGE_TIMEOUT_SECS="600"

agt-configure --load-env
```

**Option 2: CLI flags**

```bash
agt-configure --agtuuid my-agent --port 8080 --host 0.0.0.0 --secret mypassword
agt-configure --workers 4 --log-level-app info --log-level-api error
agt-configure --peer-timeout-secs 60 --peer-refresh-secs 30 --max-weight 600
agt-configure --ticket-timeout-secs 600 --message-timeout-secs 600
agt-configure --client-local
```

### Peer Discovery

```bash
# Standard (bidirectional) peer discovery — runs after 10 seconds
agt-control discover http://peer:8080/mpi --delay 10 &

# Polling peer discovery — this agent polls the peer rather than relying on callbacks
agt-control discover http://peer:8080/mpi --polling --delay 10 &
```

Use `--polling` when the remote peer cannot reach this agent directly. For example, if agent r4 can reach r3 but r3 cannot reach r4, r4 should use `--polling` so it initiates all communication.

### Starting the Server

```bash
agt-server
```

Logs are written to stderr/stdout only. Use an external tool such as logrotate or systemd-journalctl for log management. The provided Docker and systemd configurations demonstrate both approaches.

### Full Example (single agent)

```bash
cargo build --release
agt-configure --agtuuid agent-a --port 8080 --host 0.0.0.0 --secret mypassword
agt-configure --client-local
agt-control discover http://agent-b:8080/mpi --delay 10 &
agt-server
```

## Docker

The included `docker-compose.yml` spins up a five-node mesh (`r1`–`r5`) for local testing. Each service configures itself at startup using `agt-configure`, runs `agt-control discover` in the background to establish peers, and manages log rotation with logrotate:

```bash
docker compose up --build
```

The Dockerfile uses a two-stage build: a `rust:slim` builder compiles the release binaries, and a `ubuntu:24.04` runtime image contains only the binaries plus their shared-library dependencies (`libssl3`, `libsqlite3-0`) and logrotate.

## Testing

Tests use the standard Rust test harness. The test suite is protocol-compatible with stembot-python, which can be pointed at a running Rust agent to perform integration testing.

```bash
# Run all tests
cargo test

# Run tests with output visible
cargo test -- --nocapture
```

Dev dependencies include `tempfile` for ephemeral file fixtures and `mockito` for HTTP mocking.

## Protocol Architecture

### Overview

StemBot uses a layered protocol stack for distributed agent communication. The stack consists of three primary message types that work together to enable peer-to-peer network communication with asynchronous request handling and automatic routing.

### Protocol Stack Layers

```
┌─────────────────────────────────────────┐
│      CLI Tools (configure/control)      │
│  - Configure local agent settings       │
│  - Send control requests to agents      │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│       ControlForm (Request/Response)    │
│  - CreatePeer, DiscoverPeer             │
│  - DeletePeers, LoadFile, WriteFile     │
│  - SyncProcess, GetConfig, GetRoutes    │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│      NetworkMessage (Routing Layer)     │
│  - NetworkTicket (async delivery)       │
│  - Ping, Advertisement, Acknowledgement │
│  - Routes messages between peers        │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│    Actix-Web HTTP Server (Transport)    │
│  - /control endpoint (ControlForm)      │
│  - /mpi endpoint (NetworkMessage)       │
│  - AES-256 encryption per request       │
└─────────────────────────────────────────┘
```

### Message Schema

#### ControlForm

**Concrete Types:**
- `CreatePeer` — Establish peer connection with known agent UUID and URL
- `DiscoverPeer` — Discover peer by URL and automatically retrieve its UUID
- `DeletePeers` — Remove one or all peer relationships
- `GetPeers` — Retrieve list of connected peers
- `GetRoutes` — Retrieve routing table (known paths through network)
- `GetConfig` — Retrieve agent configuration (excluding encryption key)
- `SyncProcess` — Execute a command synchronously and retrieve output
- `LoadFile` — Load file from remote agent (compressed and encoded)
- `WriteFile` — Write file to remote agent (compressed and encoded)

**Wrapper Type:** `ControlFormTicket`
- Wraps a ControlForm with ticket metadata for asynchronous delivery
- Tracks UUID (`tckuuid`), source, destination, and service time
- Supports path tracing through the network

#### NetworkMessage

**Concrete Types:**
- `Ping` — Test connectivity to a peer
- `Advertisement` — Broadcast known routes to peers
- `Acknowledgement` — Confirm receipt of a message (with optional error)
- `NetworkTicket` — Async delivery container for ControlForms
- `NetworkMessagesRequest` — Poll peer for pending messages
- `NetworkMessagesResponse` — Return list of pending messages
- `TicketTraceResponse` — Report ticket hop through this agent

**Key Fields:**
- `type` — Message type enumeration
- `src` — Source agent UUID (originator)
- `isrc` — Immediate source (last agent before this one)
- `dest` — Destination agent UUID (`None` = broadcast)
- `timestamp` — Unix timestamp of creation

### CLI Tools

#### `agt-configure` — Offline Configuration

Pre-configures agent settings before startup. Settings are persisted to the local SQLite key-value store (`kvstore.sqlite`).

```bash
# View current configuration
agt-configure --view

# Set individual values
agt-configure --agtuuid my-agent --port 8080

# Load from environment variables
agt-configure --load-env

# Set client URL to localhost
agt-configure --client-local
```

#### `agt-control` — Online Agent Management

Sends control requests to a running agent.

```bash
# Discover a new peer
agt-control discover http://agent-b:8080/mpi

# Manage peers
agt-control delete --agtuuid agent-b-uuid
agt-control delete --all

# Agent statistics (config, peers, routes, hops)
agt-control stat r5

# Execute a remote command
agt-control run r5 "ls -la"

# File transfer
agt-control put /local/path /remote/path --dst-agtuuid r5

# Performance benchmark (multiple file sizes, latency + throughput)
agt-control bench r5
```

### Encryption and Security

Each request/response pair is encrypted end-to-end using AES-256 in EAX mode:

**Request Headers:**
```
Nonce:        base64(random_nonce)
Tag:          base64(aes_authentication_tag)
Content-Type: application/octet-stream
```

**Payload:**
```
base64(AES.encrypt(binary_data))
```

The encryption key is derived from `SHA-256(secret)` and must be 32 bytes. Defaults to `SHA-256("changeme")`. **Change this in production.**

### In-Memory Collections

Unlike stembot-python, the Rust implementation holds several hot collections in named in-memory SQLite databases (using `file:?mode=memory` URIs) rather than on-disk files. This eliminates disk I/O for the most frequently accessed data:

| Collection | Storage   | Description                         |
|------------|-----------|-------------------------------------|
| messages   | in-memory | Pending network messages            |
| tickets    | in-memory | Active control form tickets         |
| traces     | in-memory | Ticket trace responses              |
| routes     | in-memory | Routing table                       |
| peers      | file      | Peer relationships (`peers.sqlite`) |
| kvstore    | file      | Configuration (`kvstore.sqlite`)    |

Each collection is a process-wide singleton (`OnceLock`) wrapped in `Arc<Mutex<Connection>>`, so all threads share a single connection with no connection overhead per request.

### Logging

The server logs exclusively to stdout/stderr via `env_logger`. Log rotation is handled externally:

- **Docker** — logrotate runs on an hourly schedule inside the container (`/etc/logrotate.d/agt-server`); `agt-server` output is piped with `tee` to `/log/agt-server.log`
- **Systemd** — the provided `agt-server.service` unit captures stdout/stderr into the journal; use `journalctl -u agt-server` to view logs

## Contributing

1. Fork it!
2. Create your feature branch: `git checkout -b my-new-feature`
3. Commit your changes: `git commit -am 'Add some feature'`
4. Push to the branch: `git push origin my-new-feature`
5. Submit a pull request

## License

MIT License

Copyright (c) 2023 Justin L. Dierking

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.