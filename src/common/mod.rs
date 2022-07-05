pub(in crate) mod frame;
mod inverted_future;
pub mod tube;

pub use inverted_future::InvertedFuture;
pub use inverted_future::InvertedFutureResolver;

#[allow(dead_code)]
#[derive(Copy,Clone,Debug)]
pub enum PeerType {
    Client,
    Server,
}
