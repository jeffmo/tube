use futures;
use crate::event::ServerEvent;
use crate::event::ServerEvent_StreamError;
use crate::event::ServerEventStateMachine;
use crate::event::ServerEventStateMachineTransitionResult;

pub struct ServerStream {
    event_state_machine: ServerEventStateMachine,
    stream_has_errored: bool,
}
impl ServerStream {
    pub fn new() -> Self {
        ServerStream {
            event_state_machine: ServerEventStateMachine::new(),
            stream_has_errored: false,
        }
    }
}
impl futures::stream::Stream for ServerStream {
    type Item = ServerEvent;

    fn poll_next(
        mut self: core::pin::Pin<&mut Self>,
        _cx: &mut futures::task::Context,
    ) -> futures::task::Poll<Option<Self::Item>> {
        if self.stream_has_errored {
            return futures::task::Poll::Ready(None)
        }

        // TODO: Determine the event by decoding the Streamr frame
        //       (For now we're hardcoding until we can decide on an http lib)
        let event = ServerEvent::AuthenticatedAndReady;

        match self.event_state_machine.transition_to(&event) {
            ServerEventStateMachineTransitionResult::Valid => {
                futures::task::Poll::Ready(Some(event))
            },
            ServerEventStateMachineTransitionResult::InvalidContinue(_from, _to) => {
                // TODO: Print some kind of error?
                futures::task::Poll::Ready(Some(event))
            },
            ServerEventStateMachineTransitionResult::InvalidFatal(from, to) => {
                // TODO: Print some kind of error?
                self.stream_has_errored = true;

                let error_event = ServerEvent::StreamError(
                    ServerEvent_StreamError::InvalidServerEventTransition(from, to)
                );
                futures::task::Poll::Ready(Some(error_event))
            }
        }
    }
}
