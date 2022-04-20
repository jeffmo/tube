#[derive(Clone, Debug, PartialEq)]
pub enum DrainReason {}

#[derive(Clone, Debug, PartialEq)]
#[allow(non_camel_case_types)]
pub enum StreamrEvent_StreamError {
  InvalidStreamrEventTransition(StreamrEventTag, StreamrEventTag),
}

#[derive(Clone, Debug, PartialEq)]
pub enum StreamrEvent {
    AuthenticatedAndReady,
    Payload(Vec<u8>),
    ClientHasFinishedSending,
    StreamError(StreamrEvent_StreamError),
    ServerMustDrain(DrainReason),
}

// TODO: Is there a way to macro-ize this so StreamrEvent and 
//       StreamrEventTag aren't so copypasta?
#[derive(Clone, Debug, PartialEq)]
pub enum StreamrEventTag {
    Uninitialized,
    AuthenticatedAndReady,
    Payload,
    ClientHasFinishedSending,
    StreamError,
    ServerMustDrain,
}
impl From<&StreamrEvent> for StreamrEventTag {
  fn from(event: &StreamrEvent) -> Self {
    match event {
      StreamrEvent::AuthenticatedAndReady => StreamrEventTag::AuthenticatedAndReady,
      StreamrEvent::Payload(_) => StreamrEventTag::Payload,
      StreamrEvent::ClientHasFinishedSending => StreamrEventTag::ClientHasFinishedSending,
      StreamrEvent::StreamError(_) => StreamrEventTag::StreamError,
      StreamrEvent::ServerMustDrain(_) => StreamrEventTag::ServerMustDrain,
    }
  }
}
