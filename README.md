# Rust IRC/Chatserver

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

### Usage

Option 1: Download the latest stable release from the [releases](https://github.com/jojeecs/chatserver-rs/releases) page, run `server` and then `client` for each connection.

Option 2: 
If you're interested in modifying the code, you can clone the repo:

`git clone github.com/jojeecs/chatserver-rs.git`

Client and Server binary's and source code are stored in their respective folders, and any utility functions are in `common`
