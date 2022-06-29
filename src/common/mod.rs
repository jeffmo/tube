pub mod frame;
pub mod tube;

#[allow(dead_code)]
#[derive(Copy,Clone,Debug)]
pub enum PeerType {
    Client,
    Server,
}
