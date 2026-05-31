# Rust TCP Chatserver
Simple IRC server & client application. 
## Table of Contents
- [Features](#features)
- [Usage](#usage)

# Features
- Multi-client support
- (Soon) Message encryption
- Private messaging
- (Soon) Private rooms/channels
- Web server (redirects to my personal website)

# Usage
To run the server/client, head to the [releases](https://github.com/jojeecs/chatserver-rs/releases) page and download client & server. 
The server runs in one terminal, and clients must be run in separate terminals. 

To build it yourself or edit any of the source files, clone & enter the repo by doing 
```
git clone https://github.com/jojeecs/chatserver-rs.git 
cd chatserver-rs 
```
The `server` and `client` folders house the server & client applications respectively.

The server listens on `127.0.0.1:8080`. 
