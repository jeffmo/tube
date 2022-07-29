mod inverted_future;
mod unique_id_manager;

pub mod frame;
pub use inverted_future::InvertedFuture;
pub use inverted_future::InvertedFutureResolver;
pub mod tube;
pub use unique_id_manager::UniqueId;
pub use unique_id_manager::UniqueIdError;
pub use unique_id_manager::UniqueIdManager;

#[allow(dead_code)]
#[derive(Copy,Clone,Debug)]
pub enum PeerType {
    Client,
    Server,
}
