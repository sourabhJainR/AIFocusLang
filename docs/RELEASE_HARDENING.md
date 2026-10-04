# Release hardening checklist

Ardisa should not be presented as self-hosting until every item below is evidenced.

- CI green on the release commit.
- Supported language subset documented.
- Compiler protocol version documented.
- Bootstrap stage status recorded without overstating self-hosting.
- Generated-program benchmark fingerprint reproducible.
- No external agent framework is required for build, test, or runtime.
- Persistent learning files are versioned and backward-readable where supported.
- Unsafe Rust boundaries are explicit and limited to documented contracts.
- Known semantic and backend gaps remain visible in the artifact gap matrix.