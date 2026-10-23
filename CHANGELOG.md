# Changelog

All notable changes to this project will be documented in this file.

## [3.0.1] - 2026-10-23

### Fixed
- `KeyType` (and therefore `KeyMetadata`/`Key`) serialized as `USER`/`AUTO`/`DIST`/`UNKNOWN`, which did not match stembot-python's lowercase `StrEnum` wire format (`user`/`auto`/`dist`/`unknown`); broad cross-implementation integration testing caught the resulting interop failures in `get_key_metadata` and stored key records. `KeyType`'s `Display` impl (used only for the key-wrapping hash) is unchanged and still renders uppercase.
- `CreateUserKey.passphrase` now serializes/deserializes as `Option<String>` (`null` when absent) instead of defaulting to an empty string, matching stembot-python's `str | None` field.

### Added
- Unit tests in `models::control` and `models::crypto` covering `CreateUserKey` and `GetKeyMetadata` serialization/deserialization, ported from stembot-python's updated protocol contract tests.
- `agt-control test --verbose/-v` and `--print-on-failure/-f` flags to pretty-print tickets, ported from stembot-python's `agt-control test` command.

### Changed
- `scripts/agt-control.sh` now takes a Docker Compose service name as its first argument (e.g. `./scripts/agt-control.sh r1 stat r5`) instead of always targeting `r1`, and uses the `docker compose` (v2) plugin instead of the legacy `docker-compose` binary.

## [3.0.0] - 2026-10-20

### Changed
- **Breaking: removed the shared encryption secret.** `secret_digest`, `AGT_SECRET`, and `agt-configure --secret` are removed. Requests now identify their key with a `Key-UUID` header.
- HTTP clients resolve keys for the destination peer, preferring distributed keys over user keys and auto keys. Rejected distributed keys are removed after HTTP 401 responses.
- Benchmark output now reports paired outbound/inbound values on one row for each payload size and concurrency.

### Added
- Per-agent key management with 200,000-round PBKDF2-HMAC-SHA256 user keys, encrypted key records, 90-day auto keys, and key metadata.
- `ExchangeKeys` network messages and a scheduled task that exchanges and rotates keys with directly connected peers.
- `agt-configure --user-key-objuuid/--user-key-passphrase/--user-key-agtuuid` and matching environment variables.
- `agt-control set-user-key` and `agt-control test` subcommands.
- `scripts/agt-control.sh` for running `agt-control` in the `r1` Docker Compose container.

### Migration
- Provision a matching user-key UUID and passphrase out-of-band on directly connected agents before discovery. The Docker Compose mesh demonstrates environment and CLI provisioning.
- Protect `stembot-master.key` and the `keys.sqlite` database; both are required to retain encrypted key records.
- Rust key storage is implementation-local; Python/Rust compatibility is provided over the wire, not by sharing local key databases.

## [2.1.1] - 2026-08-01

### Changed
- Bumped project version from `2.1.0` to `2.1.1`.
- Separated filtering functionality from `pull_network_messages`.
- Added `pull_filtered_network_messages` and `filter_network_messages` to `messaging`.

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