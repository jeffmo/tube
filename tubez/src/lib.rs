#![feature(map_try_insert)]

// "client"-feature exports
#[cfg(feature = "client")] pub mod client;
#[cfg(feature = "client")] pub use client::Client;

// "server"-feature exports
#[cfg(feature = "server")] pub mod server;
#[cfg(feature = "server")] pub use server::Server;

mod common;
