# Private Renderer Protocol

This crate defines the private protocol between `phi_recorder` and
`phi-renderer-host`. It is not a public CLI or a public application protocol.

## Frame format

Each frame is written as a fixed 24-byte little-endian header followed by a
payload:

| Offset | Size | Meaning |
| ---: | ---: | --- |
| 0 | 4 | ASCII magic `PHIR` |
| 4 | 2 | Protocol version |
| 6 | 2 | Message type |
| 8 | 8 | Request ID |
| 16 | 4 | Payload length |
| 20 | 4 | Flags |

The maximum payload is 64 MiB. A clean EOF before a new frame is a normal
shutdown condition; a truncated header or payload is a protocol error.

The renderer host must keep logs on stderr. stdout is reserved for protocol
frames so the parent can distinguish protocol corruption from diagnostics.
