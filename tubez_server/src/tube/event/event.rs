#[derive(Clone, Debug, PartialEq)]
pub enum DrainReason {}

#[derive(Clone, Debug, PartialEq)]
#[allow(non_camel_case_types)]
pub enum TubeEvent_StreamError {
  InvalidTubeEventTransition(TubeEventTag, TubeEventTag),
  ServerError(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum TubeEvent {
    AuthenticatedAndReady,
    Payload(Vec<u8>),
    ClientHasFinishedSending,
    StreamError(TubeEvent_StreamError),
    ServerMustDrain(DrainReason),
}

// TODO: Is there a way to macro-ize this so TubeEvent and 
//       TubeEventTag aren't so copypasta?
#[derive(Clone, Debug, PartialEq)]
pub enum TubeEventTag {
    Uninitialized,
    AuthenticatedAndReady,
    Payload,
    ClientHasFinishedSending,
    StreamError,
    ServerMustDrain,
}
impl From<&TubeEvent> for TubeEventTag {
  fn from(event: &TubeEvent) -> Self {
    match event {
      TubeEvent::AuthenticatedAndReady => TubeEventTag::AuthenticatedAndReady,
      TubeEvent::Payload(_) => TubeEventTag::Payload,
      TubeEvent::ClientHasFinishedSending => TubeEventTag::ClientHasFinishedSending,
      TubeEvent::StreamError(_) => TubeEventTag::StreamError,
      TubeEvent::ServerMustDrain(_) => TubeEventTag::ServerMustDrain,
    }
  }
}
