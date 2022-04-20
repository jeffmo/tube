use futures;
use std::collections::VecDeque;

use crate::streamr::event;
use crate::streamr::event::StreamrEvent;
use crate::streamr::event::StreamrEvent_StreamError;

pub struct Streamr {
    event_state_machine: event::StateMachine,
    stream_has_ended: bool,
    test_events: Option<VecDeque<StreamrEvent>>,
}
impl Streamr {
    pub fn new() -> Self {
        Streamr {
            event_state_machine: event::StateMachine::new(),
            stream_has_ended: false,
            test_events: None,
        }
    }

    fn get_next_event(&mut self) -> Option<StreamrEvent> {
        match self.test_events.as_mut() {
            None => Some(StreamrEvent::AuthenticatedAndReady),
            Some(events) => events.pop_front(),
        }
    }
}
impl futures::stream::Stream for Streamr {
    type Item = StreamrEvent;

    fn poll_next(
        mut self: core::pin::Pin<&mut Self>,
        _cx: &mut futures::task::Context,
    ) -> futures::task::Poll<Option<Self::Item>> {
        if self.stream_has_ended {
            return futures::task::Poll::Ready(None)
        }

        // TODO: Determine the event by decoding the Streamr frame
        //       (For now we're hardcoding until we can decide on an http lib)
        match self.get_next_event() {
            None => {
                self.stream_has_ended = true;
                futures::task::Poll::Ready(None)
            },

            Some(event) => match self.event_state_machine.transition_to(&event) {
                event::StateMachineTransitionResult::Valid => {
                    futures::task::Poll::Ready(Some(event))
                },
                event::StateMachineTransitionResult::Invalid(from, to) => {
                    // TODO: Print some kind of error?
                    self.stream_has_ended = true;

                    let error_event = StreamrEvent::StreamError(
                        StreamrEvent_StreamError::InvalidStreamrEventTransition(from, to)
                    );
                    futures::task::Poll::Ready(Some(error_event))
                }
            }
        }
    }
}

#[cfg(test)]
impl Streamr {
    pub fn set_test_events(&mut self, events: Vec<StreamrEvent>) {
        self.test_events = Some(VecDeque::from(events))
    }
}

#[cfg(test)]
mod streamr_tests {
    use futures::StreamExt;
    use tokio;

    use super::event::StreamrEvent;
    use super::event::StreamrEvent_StreamError;
    use super::event::StreamrEventTag;
    use super::Streamr;

    #[tokio::test]
    async fn emits_valid_initial_event() {
        let mut stream = Streamr::new();
        let test_events = vec![
            StreamrEvent::AuthenticatedAndReady,
        ];
        let expected_events = test_events.clone();

        stream.set_test_events(test_events);
        let actual_events = stream.collect::<Vec<StreamrEvent>>().await;
        assert_eq!(actual_events, expected_events);
    }

    #[tokio::test]
    async fn emits_error_on_payload_before_authenticated() {
        let mut stream = Streamr::new();
        let test_events = vec![
            StreamrEvent::Payload(vec![]),
        ];
        let expected_events = vec![
            StreamrEvent::StreamError(
                StreamrEvent_StreamError::InvalidStreamrEventTransition(
                    StreamrEventTag::Uninitialized,
                    StreamrEventTag::Payload,
                )
            )
        ];

        stream.set_test_events(test_events);
        let actual_events = stream.collect::<Vec<StreamrEvent>>().await;
        assert_eq!(actual_events, expected_events);
    }

    #[tokio::test]
    async fn emits_error_on_payload_after_clienthasfinished() {
        let mut stream = Streamr::new();
        let test_events = vec![
            StreamrEvent::AuthenticatedAndReady,
            StreamrEvent::ClientHasFinishedSending,
            StreamrEvent::Payload(vec![]),
        ];

        let mut expected_events = test_events.clone();
        // Delete the Payload event from the end...
        expected_events.pop();
        // Push on a StreamError event...
        expected_events.push(
            StreamrEvent::StreamError(
                StreamrEvent_StreamError::InvalidStreamrEventTransition(
                    StreamrEventTag::ClientHasFinishedSending,
                    StreamrEventTag::Payload,
                )
            )
        );

        stream.set_test_events(test_events);
        let actual_events = stream.collect::<Vec<StreamrEvent>>().await;
        assert_eq!(actual_events, expected_events);
    }

    #[tokio::test]
    async fn terminates_stream_on_first_erroneous_event() {
        let mut stream = Streamr::new();
        let test_events = vec![
            StreamrEvent::AuthenticatedAndReady,
            StreamrEvent::ClientHasFinishedSending,
            StreamrEvent::Payload(vec![]),
            StreamrEvent::Payload(vec![]),
        ];

        let mut expected_events = test_events.clone();
        // Delete both Payload events from the end...
        expected_events.pop();
        expected_events.pop();

        // Push on a StreamError event...
        expected_events.push(
            StreamrEvent::StreamError(
                StreamrEvent_StreamError::InvalidStreamrEventTransition(
                    StreamrEventTag::ClientHasFinishedSending,
                    StreamrEventTag::Payload,
                )
            )
        );

        stream.set_test_events(test_events);
        let actual_events = stream.collect::<Vec<StreamrEvent>>().await;
        assert_eq!(actual_events, expected_events);
    }
}
