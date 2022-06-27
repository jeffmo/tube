#![feature(map_try_insert)]

mod common;

pub use common::tube;

// "client"-feature exports
#[cfg(feature = "client")] pub mod client;
#[cfg(feature = "client")] pub use client::Client;

// "server"-feature exports
#[cfg(feature = "server")] pub mod server;
#[cfg(feature = "server")] pub use server::Server;
