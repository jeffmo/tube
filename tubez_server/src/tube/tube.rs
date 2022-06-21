use futures;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Waker;

use tubez_common::frame;

use crate::tube::event;
use crate::tube::event::TubeEvent;
use crate::tube::event::TubeEvent_StreamError;
use crate::tube::TubeManager;

#[derive(Debug)]
pub enum SendError {
    TransportError(hyper::Error),
    FrameEncodeError(frame::FrameEncodeError),
    UnknownTransportError,
}

#[derive(Debug)]
pub struct Tube {
    ack_id_counter: u16,
    sender: Arc<Mutex<hyper::body::Sender>>,
    tube_mgr: Arc<Mutex<TubeManager>>,
    tube_id: u16,
}
impl Tube {
    pub(in crate) fn new(
        tube_id: u16,
        sender: Arc<Mutex<hyper::body::Sender>>, 
        tube_mgr: Arc<Mutex<TubeManager>>,
    ) -> Self {
        Tube {
            ack_id_counter: 0,
            sender,
            tube_mgr,
            tube_id,
        }
    }

    pub async fn send(&mut self, data: Vec<u8>) -> Result<(), SendError> {
        let ack_id = self.ack_id_counter;
        self.ack_id_counter += 1;
        match frame::encode_payload_frame(self.tube_id, Some(ack_id), data) {
            Ok(frame_data) => {
                {
                    let mut sender = self.sender.lock().unwrap();
                    match sender.send_data(frame_data.into()).await {
                        Ok(_) => (),
                        Err(e) => return Err(SendError::TransportError(e)),
                    }
                };

                // TODO: Await here for the return of an ACK from the client

                Ok(())
            },
            Err(e) => Err(SendError::FrameEncodeError(e)),
        }
    }

    pub fn send_and_forget(&mut self, data: Vec<u8>) -> Result<(), SendError> {
        match frame::encode_payload_frame(self.tube_id, None, data) {
            Ok(frame_data) => {
                let mut sender = self.sender.lock().unwrap();
                match sender.try_send_data(frame_data.into()) {
                    Ok(_) => Ok(()),
                    Err(_bytes) => Err(SendError::UnknownTransportError),
                }
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
        //let mut event_queue = self.event_queue.lock().unwrap();
        let mut tube_mgr = self.tube_mgr.lock().unwrap();
        tube_mgr.waker = Some(cx.waker().clone());

        if tube_mgr.terminated {
            return futures::task::Poll::Ready(None);
        }

        match tube_mgr.pending_events.pop_front() {
            Some(tube_event) => match tube_mgr.state_machine.transition_to(&tube_event) {
                event::StateMachineTransitionResult::Valid => {
                    futures::task::Poll::Ready(Some(tube_event))
                },
                event::StateMachineTransitionResult::Invalid(from, to) => {
                    // TODO: Print some kind of error?
                    tube_mgr.terminated = true;

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
        let mut tube_mgr = self.tube_mgr.lock().unwrap();
        tube_mgr.pending_events = VecDeque::from(events);
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
