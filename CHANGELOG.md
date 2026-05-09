# Changelog

All notable changes to this project will be documented in this file.

## [1.0.1] - 2026-05-09

### Changed
- Updated to rust 1.95
- Updated and refactored dependency versions for compatibility.
- Reduced default dependency feature sets to narrow transitive dependencies.
- Automatic collection vacuum.
- Corrected topology in docker compose file.
- "debian" feature to place sqlite files in /var/agt for installed agents.