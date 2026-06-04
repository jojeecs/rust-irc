# ChatServer-RS

A high-performance, asynchronous chat application built with Rust, featuring secure authentication and persistent storage.

## Features

- **Asynchronous Architecture**: Built on top of `tokio` for efficient handling of concurrent connections.
- **Persistent SQL Storage**: Utilizes `turso` for reliable user data management.
- **Secure Authentication**: Implements password-based login with SHA3-256 hashing.
- **Interactive CLI**: Enhanced user experience using the `cliclack` library for terminal-based interactions.
- **Messaging System**:
    - **Public Broadcasts**: Send messages to everyone connected to the server.
    - **Private Messaging**: Secure one-on-one communication between users.
- **Protocol**: Custom packet handling with JSON serialization/deserialization for reliable data transfer.
- **Server Administration**: Direct admin input for server-side commands and management.
## Getting Started

### Prerequisites

- Rust (latest stable version)
- SQLite (for local development)

### Running the Server

```bash
cargo run --bin server
```

### Running the Client

```bash
cargo run --bin client
```