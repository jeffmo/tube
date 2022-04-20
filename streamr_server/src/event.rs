#[derive(Clone, Debug, PartialEq)]
pub enum DrainReason {}

#[derive(Clone, Debug, PartialEq)]
#[allow(non_camel_case_types)]
pub enum ServerEvent_StreamError {
  InvalidServerEventTransition(ServerEventTag, ServerEventTag),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ServerEvent {
    AuthenticatedAndReady,
    Payload(Vec<u8>),
    ClientHasFinishedSending,
    StreamError(ServerEvent_StreamError),
    ServerMustDrain(DrainReason),
}

// TODO: Is there a way to macro-ize this so ServerEvent and 
//       ServerEventTag aren't so copypasta?
#[derive(Clone, Debug, PartialEq)]
pub enum ServerEventTag {
    Uninitialized,
    AuthenticatedAndReady,
    Payload,
    ClientHasFinishedSending,
    StreamError,
    ServerMustDrain,
}
impl From<&ServerEvent> for ServerEventTag {
  fn from(event: &ServerEvent) -> Self {
    match event {
      ServerEvent::AuthenticatedAndReady => ServerEventTag::AuthenticatedAndReady,
      ServerEvent::Payload(_) => ServerEventTag::Payload,
      ServerEvent::ClientHasFinishedSending => ServerEventTag::ClientHasFinishedSending,
      ServerEvent::StreamError(_) => ServerEventTag::StreamError,
      ServerEvent::ServerMustDrain(_) => ServerEventTag::ServerMustDrain,
    }
  }
}

pub enum ServerEventStateMachineTransitionResult {
  Valid,
  Invalid(ServerEventTag, ServerEventTag), 
}

pub struct ServerEventStateMachine {
  prev_event_tag: ServerEventTag,
}
impl ServerEventStateMachine {
  pub fn new() -> Self {
    ServerEventStateMachine {
      prev_event_tag: ServerEventTag::Uninitialized,
    }
  }

  pub fn transition_to(&mut self, next_event: &ServerEvent) 
    -> ServerEventStateMachineTransitionResult {
    let next_event_tag = ServerEventTag::from(next_event);

    // TODO: Mayhaps a fancy StateMachine crate lib could be created with a 
    //       little less verbose macro syntax to describe the state machine?
    {
      use ServerEventTag::*;
      match (&self.prev_event_tag, &next_event_tag) {
        (Uninitialized, AuthenticatedAndReady) => {
          self.prev_event_tag = next_event_tag;
          ServerEventStateMachineTransitionResult::Valid
        },

        (AuthenticatedAndReady, 
          Payload | ClientHasFinishedSending | StreamError | ServerMustDrain) => {
          self.prev_event_tag = next_event_tag;
          ServerEventStateMachineTransitionResult::Valid
        },

        (Payload,
          Payload | ClientHasFinishedSending | StreamError | ServerMustDrain) => {
          self.prev_event_tag = next_event_tag;
          ServerEventStateMachineTransitionResult::Valid
        },

        (ClientHasFinishedSending,
          StreamError | ServerMustDrain) => {
          self.prev_event_tag = next_event_tag;
          ServerEventStateMachineTransitionResult::Valid
        },

        (ServerMustDrain, 
          Payload | ClientHasFinishedSending | StreamError) => {
          self.prev_event_tag = next_event_tag;
          ServerEventStateMachineTransitionResult::Valid
        },

        // TODO: Fill this out to cover exhaustive list of transitions

        // InvalidFatal fallthrough
        (_, _) => {
          ServerEventStateMachineTransitionResult::Invalid(
            self.prev_event_tag.clone(), next_event_tag)
        },
      }
    }
  }
}
