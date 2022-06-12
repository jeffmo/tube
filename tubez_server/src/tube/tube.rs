use futures;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Waker;

use tubez_common::frame;

use crate::tube::event;
use crate::tube::event::TubeEvent;
use crate::tube::event::TubeEvent_StreamError;

#[derive(Debug)]
pub enum SendError {
    TransportError(hyper::Error),
    FrameEncodeError(frame::FrameEncodeError),
    UnknownTransportError,
}

struct TubeEventQueue {
    // TODO: Merge this in with the event statemachine...?
    terminated: bool,
    pending_events: VecDeque<TubeEvent>,
    state_machine: event::StateMachine,
    waker: Option<Waker>,
}

pub struct Tube {
    ack_id_counter: u16,
    // TODO: Does this really need to be Arc<Mutex> since all TubeEvents stay 
    //       on a single thread?
    event_queue: Arc<Mutex<TubeEventQueue>>,
    sender: hyper::body::Sender,
    tube_id: u16,
}
impl Tube {
    pub(in crate) fn new(
        tube_id: u16,
        sender: hyper::body::Sender, 
    ) -> Self {
        Tube {
            ack_id_counter: 0,
            event_queue: Arc::new(Mutex::new(TubeEventQueue {
                pending_events: VecDeque::new(),
                state_machine: event::StateMachine::new(),
                terminated: false,
                waker: None,
            })),
            sender,
            tube_id,
        }
    }

    pub(in crate) fn handle_ack_response(&mut self, ack_id: u16) {
        // TODO 
    }

    pub async fn send(&mut self, data: Vec<u8>) -> Result<(), SendError> {
        let ack_id = self.ack_id_counter;
        self.ack_id_counter += 1;
        match frame::encode_payload_frame(self.tube_id, Some(ack_id), data) {
            Ok(frame_data) => match self.sender.send_data(frame_data.into()).await {
                Ok(_) => {
                    // Await for the ack before returning
                    Ok(())
                },
                Err(e) => Err(SendError::TransportError(e)),
            },
            Err(e) => Err(SendError::FrameEncodeError(e)),
        }
    }

    pub fn send_and_forget(&mut self, data: Vec<u8>) -> Result<(), SendError> {
        match frame::encode_payload_frame(self.tube_id, None, data) {
            Ok(frame_data) => match self.sender.try_send_data(frame_data.into()) {
                Ok(_) => Ok(()),
                Err(_bytes) => Err(SendError::UnknownTransportError),
            },
            Err(e) => Err(SendError::FrameEncodeError(e)),
        }
    }
}
impl futures::stream::Stream for Tube {
    type Item = TubeEvent;

    fn poll_next(
        self: core::pin::Pin<&mut Self>,
        cx: &mut futures::task::Context,
    ) -> futures::task::Poll<Option<Self::Item>> {
        let mut event_queue = self.event_queue.lock().unwrap();
        event_queue.waker = Some(cx.waker().clone());

        if event_queue.terminated {
            return futures::task::Poll::Ready(None);
        }

        match event_queue.pending_events.pop_front() {
            Some(tube_event) => match event_queue.state_machine.transition_to(&tube_event) {
                event::StateMachineTransitionResult::Valid => {
                    futures::task::Poll::Ready(Some(tube_event))
                },
                event::StateMachineTransitionResult::Invalid(from, to) => {
                    // TODO: Print some kind of error?
                    event_queue.terminated = true;

                    let error_event = TubeEvent::StreamError(
                        TubeEvent_StreamError::InvalidTubeEventTransition(from, to)
                    );
                    futures::task::Poll::Ready(Some(error_event))
                }
            },

            None => futures::task::Poll::Pending
        }
    }
}

#[cfg(test)]
impl Tube {
    pub fn set_test_events(&mut self, events: Vec<TubeEvent>) {
        let mut event_queue = self.event_queue.lock().unwrap();
        event_queue.pending_events = VecDeque::from(events);
    }
}

#[cfg(test)]
mod tube_tests {
    use futures::StreamExt;
    use hyper;
    use tokio;

    use super::event::TubeEvent;
    use super::event::TubeEvent_StreamError;
    use super::event::TubeEventTag;
    use super::Tube;

    #[tokio::test]
    async fn emits_valid_initial_event() {
        let (sender, _body) = hyper::Body::channel();
        let mut stream = Tube::new(42, sender);
        let test_events = vec![
            TubeEvent::AuthenticatedAndReady,
        ];
        let expected_events = test_events.clone();

        stream.set_test_events(test_events);

        let actual_events = stream
            .take(expected_events.len())
            .collect::<Vec<TubeEvent>>().await;

        assert_eq!(actual_events, expected_events);
    }

    #[tokio::test]
    async fn emits_error_on_payload_before_authenticated() {
        let (sender, _body) = hyper::Body::channel();
        let mut stream = Tube::new(42, sender);
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
        let (sender, _body) = hyper::Body::channel();
        let mut stream = Tube::new(42, sender);
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
        let (sender, _body) = hyper::Body::channel();
        let mut stream = Tube::new(42, sender);
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
