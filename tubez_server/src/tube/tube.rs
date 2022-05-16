use futures;
use std::collections::VecDeque;

use crate::tube::event;
use crate::tube::event::TubeEvent;
use crate::tube::event::TubeEvent_StreamError;

pub struct Tube {
    event_state_machine: event::StateMachine,
    stream_has_ended: bool,
    test_events: Option<VecDeque<TubeEvent>>,
}
impl Tube {
    pub fn new() -> Self {
        Tube {
            event_state_machine: event::StateMachine::new(),
            stream_has_ended: false,
            test_events: None,
        }
    }

    fn get_next_event(&mut self) -> Option<TubeEvent> {
        match self.test_events.as_mut() {
            None => Some(TubeEvent::AuthenticatedAndReady),
            Some(events) => events.pop_front(),
        }
    }
}
impl futures::stream::Stream for Tube {
    type Item = TubeEvent;

    fn poll_next(
        mut self: core::pin::Pin<&mut Self>,
        _cx: &mut futures::task::Context,
    ) -> futures::task::Poll<Option<Self::Item>> {
        if self.stream_has_ended {
            return futures::task::Poll::Ready(None)
        }

        // TODO: Determine the event by decoding the Tube frame
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

                    let error_event = TubeEvent::StreamError(
                        TubeEvent_StreamError::InvalidTubeEventTransition(from, to)
                    );
                    futures::task::Poll::Ready(Some(error_event))
                }
            }
        }
    }
}

#[cfg(test)]
impl Tube {
    pub fn set_test_events(&mut self, events: Vec<TubeEvent>) {
        self.test_events = Some(VecDeque::from(events))
    }
}

#[cfg(test)]
mod tube_tests {
    use futures::StreamExt;
    use tokio;

    use super::event::TubeEvent;
    use super::event::TubeEvent_StreamError;
    use super::event::TubeEventTag;
    use super::Tube;

    #[tokio::test]
    async fn emits_valid_initial_event() {
        let mut stream = Tube::new();
        let test_events = vec![
            TubeEvent::AuthenticatedAndReady,
        ];
        let expected_events = test_events.clone();

        stream.set_test_events(test_events);
        let actual_events = stream.collect::<Vec<TubeEvent>>().await;
        assert_eq!(actual_events, expected_events);
    }

    #[tokio::test]
    async fn emits_error_on_payload_before_authenticated() {
        let mut stream = Tube::new();
        let test_events = vec![
            TubeEvent::Payload(vec![]),
        ];
        let expected_events = vec![
            TubeEvent::StreamError(
                TubeEvent_StreamError::InvalidTubeEventTransition(
                    TubeEventTag::Uninitialized,
                    TubeEventTag::Payload,
                )
            )
        ];

        stream.set_test_events(test_events);
        let actual_events = stream.collect::<Vec<TubeEvent>>().await;
        assert_eq!(actual_events, expected_events);
    }

    #[tokio::test]
    async fn emits_error_on_payload_after_clienthasfinished() {
        let mut stream = Tube::new();
        let test_events = vec![
            TubeEvent::AuthenticatedAndReady,
            TubeEvent::ClientHasFinishedSending,
            TubeEvent::Payload(vec![]),
        ];

        let mut expected_events = test_events.clone();
        // Delete the Payload event from the end...
        expected_events.pop();
        // Push on a StreamError event...
        expected_events.push(
            TubeEvent::StreamError(
                TubeEvent_StreamError::InvalidTubeEventTransition(
                    TubeEventTag::ClientHasFinishedSending,
                    TubeEventTag::Payload,
                )
            )
        );

        stream.set_test_events(test_events);
        let actual_events = stream.collect::<Vec<TubeEvent>>().await;
        assert_eq!(actual_events, expected_events);
    }

    #[tokio::test]
    async fn terminates_stream_on_first_erroneous_event() {
        let mut stream = Tube::new();
        let test_events = vec![
            TubeEvent::AuthenticatedAndReady,
            TubeEvent::ClientHasFinishedSending,
            TubeEvent::Payload(vec![]),
            TubeEvent::Payload(vec![]),
        ];

        let mut expected_events = test_events.clone();
        // Delete both Payload events from the end...
        expected_events.pop();
        expected_events.pop();

        // Push on a StreamError event...
        expected_events.push(
            TubeEvent::StreamError(
                TubeEvent_StreamError::InvalidTubeEventTransition(
                    TubeEventTag::ClientHasFinishedSending,
                    TubeEventTag::Payload,
                )
            )
        );

        stream.set_test_events(test_events);
        let actual_events = stream.collect::<Vec<TubeEvent>>().await;
        assert_eq!(actual_events, expected_events);
    }
}
