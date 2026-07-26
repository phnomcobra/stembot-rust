# Changelog

All notable changes to this project will be documented in this file.

## [2.1.0] - 2026-07-26

### Added
- `network_whitelist` and `control_whitelist` fields on `NetworkMessagesRequest`.
  - `network_whitelist`: optional list of network message type strings (e.g. `["ping", "ticket_request"]`); messages whose type is not in the list are dropped. Dropped `ticket_request` messages are automatically serviced with a `ticket_response` error reply (src/dest swapped, `error` set to `"… is not allowed by whitelist."`).
  - `control_whitelist`: optional list of control form type strings (e.g. `["sync_process"]`); `ticket_request` messages whose form type is not in the list are dropped and an error `ticket_response` is generated. All other message types pass through unchanged.
  - Both fields are backward-compatible: peers that omit them are handled identically to previous behaviour.
- Unit tests in `messaging` and `models::network` covering whitelist serialization, deserialization (including missing-field backward compat), and end-to-end filtering behaviour.

## [2.0.0] - 2026-05-18

### Added
- `Benchmark` control form for measuring outbound and inbound payload throughput.
  - Wire shape: `{"type":"benchmark","outbound_size":…,"inbound_size":…,"payload":…,"error":…,"objuuid":…,"coluuid":…}`
  - The responding agent fills `payload` with `inbound_size` bytes; the requesting client fills it with `outbound_size` bytes before sending. `payload` is stripped before the ticket is persisted.
- `limit` field on `NetworkMessagesRequest` to cap per-agent message pulls.
  - Wire shape: optional integer field `"limit"` in the `messages_request` network message; omitted or `null` means no cap.
  - Backward-compatible: peers that omit the field are handled identically to the previous behaviour.
- `find_limited` and `pop_limited` methods on `Collection` for bounded queries.
- `RESERVED_ATTRIBUTES` constant (`"limit"`) in the DAO layer; reserved names are rejected by `create_attribute` and `delete_attribute` and are silently skipped in query slices.
- `--help` / `-h` / `-?` flags on `agt-control` and `agt-configure` CLIs.

### Changed
- HTTP request and response bodies for both the `/control` and `/mpi` endpoints are now `Content-Type: application/binary` containing raw AES-256-EAX ciphertext. The AES nonce and MAC authentication tag are transmitted as hex-encoded strings in the `Nonce` and `Tag` HTTP headers respectively.
- `pull_network_messages` respects `request.limit`, applying the cap per destination agent UUID.
- Benchmark sub-command reports directional (OUT / IN) rows only; combined benchmark removed.

## [1.0.1] - 2026-05-09

### Changed
- Updated to rust 1.95
- Updated and refactored dependency versions for compatibility.
- Reduced default dependency feature sets to narrow transitive dependencies.
- Automatic collection vacuum.
- Corrected topology in docker compose file.
- "debian" feature to place sqlite files in /var/agt for installed agents.