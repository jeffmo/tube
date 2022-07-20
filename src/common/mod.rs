pub mod frame;
mod inverted_future;
pub mod tube;
mod unique_id_manager;

pub use inverted_future::InvertedFuture;
pub use inverted_future::InvertedFutureResolver;
pub use unique_id_manager::UniqueIdError;
pub use unique_id_manager::UniqueIdManager;

#[allow(dead_code)]
#[derive(Copy,Clone,Debug)]
pub enum PeerType {
    Client,
    Server,
}
