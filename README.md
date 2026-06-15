# Rust IRC

A high-performance, asynchronous chat application built with Rust, featuring secure encrypted communication, persistent storage, and a modern Terminal User Interface (TUI).

## Features

- **Asynchronous Architecture**: Built with `tokio` for efficient concurrent connection handling.
- **Secure Communication**: End-to-end encrypted streams using the `clavis` library.
- **Persistent Storage**: User management powered by Turso (SQLite) for reliable data persistence.
- **Modern TUI**: Interactive terminal interface built with `ratatui` and `tui-input`.
- **Flexible Messaging**:
    - **Global Broadcasts**: Chat with everyone connected to the server.
    - **Private Messaging**: Secure, direct one-on-one messages.
- **Encrypted Authentication**: SHA3-256 hashed password verification.

## Project Structure

This workspace consists of three main crates:

- **`client/`**: The TUI-based chat client.
- **`server/`**: The asynchronous chat server.
- **`common/`**: Shared data structures, protocol definitions, and domain logic.


### Prerequisites
- Rust (latest stable)
- SQLite

### Usage

1. **Clone the repository**:
   ```bash
   git clone https://github.com/jojeecs/chatserver-rs.git
   cd chatserver-rs
   ```

2. **Run the Server**:
   ```bash
   cargo run --bin server
   ```

3. **Run the Client**:
   ```bash
   cargo run --bin client
   ```
