use futures;
use std::collections::VecDeque;

use crate::event::ServerEvent;
use crate::event::ServerEvent_StreamError;
use crate::event::ServerEventStateMachine;
use crate::event::ServerEventStateMachineTransitionResult;

pub struct Streamr {
    event_state_machine: ServerEventStateMachine,
    stream_has_ended: bool,
    test_events: Option<VecDeque<ServerEvent>>,
}
impl Streamr {
    pub fn new() -> Self {
        Streamr {
            event_state_machine: ServerEventStateMachine::new(),
            stream_has_ended: false,
            test_events: None,
        }
    }

    fn get_next_event(&mut self) -> Option<ServerEvent> {
        match self.test_events.as_mut() {
            None => Some(ServerEvent::AuthenticatedAndReady),
            Some(events) => events.pop_front(),
        }
    }
}
impl futures::stream::Stream for Streamr {
    type Item = ServerEvent;

    fn poll_next(
        mut self: core::pin::Pin<&mut Self>,
        _cx: &mut futures::task::Context,
    ) -> futures::task::Poll<Option<Self::Item>> {
        if self.stream_has_ended {
            return futures::task::Poll::Ready(None)
        }

        // TODO: Determine the event by decoding the Streamr frame
        //       (For now we're hardcoding until we can decide on an http lib)
        //let event = ServerEvent::AuthenticatedAndReady;
        let next_event = self.get_next_event();
        match next_event {
            None => {
                self.stream_has_ended = true;
                futures::task::Poll::Ready(None)
            },

            Some(event) => match self.event_state_machine.transition_to(&event) {
                ServerEventStateMachineTransitionResult::Valid => {
                    futures::task::Poll::Ready(Some(event))
                },
                ServerEventStateMachineTransitionResult::Invalid(from, to) => {
                    // TODO: Print some kind of error?
                    self.stream_has_ended = true;

                    let error_event = ServerEvent::StreamError(
                        ServerEvent_StreamError::InvalidServerEventTransition(from, to)
                    );
                    futures::task::Poll::Ready(Some(error_event))
                }
            }
        }
    }
}

#[cfg(test)]
impl Streamr {
    pub fn set_test_events(&mut self, events: Vec<ServerEvent>) {
        self.test_events = Some(VecDeque::from(events))
    }
}

#[cfg(test)]
mod streamr_tests {
    use futures::StreamExt;
    use tokio;

    use crate::event::ServerEvent;
    use crate::event::ServerEvent_StreamError;
    use crate::event::ServerEventTag;
    use crate::streamr::Streamr;

    #[tokio::test]
    async fn emits_valid_initial_event() {
        let mut stream = Streamr::new();
        let test_events = vec![
            ServerEvent::AuthenticatedAndReady,
        ];
        let expected_events = test_events.clone();

        stream.set_test_events(test_events);
        let actual_events = stream.collect::<Vec<ServerEvent>>().await;
        assert_eq!(actual_events, expected_events);
    }

    #[tokio::test]
    async fn emits_error_on_payload_before_authenticated() {
        let mut stream = Streamr::new();
        let test_events = vec![
            ServerEvent::Payload(vec![]),
        ];
        let expected_events = vec![
            ServerEvent::StreamError(
                ServerEvent_StreamError::InvalidServerEventTransition(
                    ServerEventTag::Uninitialized,
                    ServerEventTag::Payload,
                )
            )
        ];

        stream.set_test_events(test_events);
        let actual_events = stream.collect::<Vec<ServerEvent>>().await;
        assert_eq!(actual_events, expected_events);
    }

    #[tokio::test]
    async fn emits_error_on_payload_after_clienthasfinished() {
        let mut stream = Streamr::new();
        let test_events = vec![
            ServerEvent::AuthenticatedAndReady,
            ServerEvent::ClientHasFinishedSending,
            ServerEvent::Payload(vec![]),
        ];

        let mut expected_events = test_events.clone();
        // Delete the Payload event from the end...
        expected_events.pop();
        // Push on a StreamError event...
        expected_events.push(
            ServerEvent::StreamError(
                ServerEvent_StreamError::InvalidServerEventTransition(
                    ServerEventTag::ClientHasFinishedSending,
                    ServerEventTag::Payload,
                )
            )
        );

        stream.set_test_events(test_events);
        let actual_events = stream.collect::<Vec<ServerEvent>>().await;
        assert_eq!(actual_events, expected_events);
    }

    #[tokio::test]
    async fn terminates_stream_on_first_erroneous_event() {
        let mut stream = Streamr::new();
        let test_events = vec![
            ServerEvent::AuthenticatedAndReady,
            ServerEvent::ClientHasFinishedSending,
            ServerEvent::Payload(vec![]),
            ServerEvent::Payload(vec![]),
        ];

        let mut expected_events = test_events.clone();
        // Delete both Payload events from the end...
        expected_events.pop();
        expected_events.pop();

        // Push on a StreamError event...
        expected_events.push(
            ServerEvent::StreamError(
                ServerEvent_StreamError::InvalidServerEventTransition(
                    ServerEventTag::ClientHasFinishedSending,
                    ServerEventTag::Payload,
                )
            )
        );

        stream.set_test_events(test_events);
        let actual_events = stream.collect::<Vec<ServerEvent>>().await;
        assert_eq!(actual_events, expected_events);
    }
}
