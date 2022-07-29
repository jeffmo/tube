use crate::common::frame;

// TODO
/*
#[derive(Clone, Debug, PartialEq)]
#[allow(non_camel_case_types)]
pub enum TubeEvent_DrainReason {}
*/

#[derive(Clone, Debug, PartialEq)]
#[allow(non_camel_case_types)]
pub enum TubeEvent_StreamError {
  InvalidTubeEventTransition(TubeEventTag, TubeEventTag),
  ServerError(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum TubeEvent {
    Abort(frame::AbortReason),
    AuthenticatedAndReady,
    ClientHasFinishedSending,
    Payload(Vec<u8>),
    StreamError(TubeEvent_StreamError),
    ServerHasFinishedSending,

    // TODO
    /*
    ServerMustDrain(TubeEvent_DrainReason),
    */
}

// TODO: Is there a way to macro-ize this so TubeEvent and 
//       TubeEventTag aren't so copypasta?
#[derive(Clone, Debug, PartialEq)]
pub enum TubeEventTag {
    Abort,
    Uninitialized,
    AuthenticatedAndReady,
    Payload,
    ClientHasFinishedSending,
    StreamError,
    ServerHasFinishedSending,

    // TODO
    /*
    ServerMustDrain,
    */
}
impl From<&TubeEvent> for TubeEventTag {
    fn from(event: &TubeEvent) -> Self {
        match event {
            TubeEvent::Abort(_) => TubeEventTag::Abort,
            TubeEvent::AuthenticatedAndReady => TubeEventTag::AuthenticatedAndReady,
            TubeEvent::Payload(_) => TubeEventTag::Payload,
            TubeEvent::ClientHasFinishedSending => TubeEventTag::ClientHasFinishedSending,
            TubeEvent::StreamError(_) => TubeEventTag::StreamError,
            TubeEvent::ServerHasFinishedSending => TubeEventTag::ServerHasFinishedSending,

            // TODO
            /*
            TubeEvent::ServerMustDrain(_) => TubeEventTag::ServerMustDrain,
            */
        }
    }
}
