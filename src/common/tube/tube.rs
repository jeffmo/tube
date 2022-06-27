use futures;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;

use hyper;

use crate::common::frame;
use super::sendack_future::SendAckFuture;
use super::sendack_future::SendAckFutureContext;
use super::tube_event;
use super::tube_event::TubeEvent;
use super::tube_event::TubeEvent_StreamError;
use super::TubeManager;

#[derive(Debug)]
pub enum SendError {
    AckIdAlreadyInUseInternalError,
    TransportError(hyper::Error),
    FrameEncodeError(frame::FrameEncodeError),
    UnknownTransportError,
}

#[derive(Debug)]
enum TubeOwner {
    Client,
    Server,
}

#[derive(Debug)]
pub struct Tube {
    available_ackids: VecDeque<u16>,
    ack_id_counter: u16,
    sendacks: HashMap<u16, Arc<Mutex<SendAckFutureContext>>>,
    sender: Arc<tokio::sync::Mutex<hyper::body::Sender>>,
    tube_id: u16,
    tube_manager: Arc<Mutex<TubeManager>>,
    tube_owner: TubeOwner,
}
impl Tube {
    fn new(
        tube_owner: TubeOwner,
        tube_id: u16,
        sender: Arc<tokio::sync::Mutex<hyper::body::Sender>>, 
        tube_manager: Arc<Mutex<TubeManager>>,
    ) -> Self {
        Tube {
            ack_id_counter: 0,
            available_ackids: VecDeque::new(),
            sendacks: HashMap::new(),
            sender,
            tube_id,
            tube_manager,
            tube_owner,
        }
    }

    #[cfg(feature = "client")]
    pub(in crate) fn new_on_client(
        tube_id: u16,
        sender: Arc<tokio::sync::Mutex<hyper::body::Sender>>, 
        tube_manager: Arc<Mutex<TubeManager>>,
    ) -> Self {
      Self::new(TubeOwner::Client, tube_id, sender, tube_manager)
    }

    #[cfg(feature = "server")]
    pub(in crate) fn new_on_server(
        tube_id: u16,
        sender: Arc<tokio::sync::Mutex<hyper::body::Sender>>, 
        tube_manager: Arc<Mutex<TubeManager>>,
    ) -> Self {
      Self::new(TubeOwner::Server, tube_id, sender, tube_manager)
    }

    fn take_ackid(&mut self) -> u16 {
        match self.available_ackids.pop_front() {
            Some(ack_id) => ack_id,
            None => {
                let ack_id = self.ack_id_counter;
                if ack_id == u16::MAX {
                    self.ack_id_counter = 0;
                } else {
                    self.ack_id_counter += 1;
                }
                ack_id
            }
        }
    }

    pub fn get_id(&self) -> u16 {
        return self.tube_id;
    }

    pub async fn send(&mut self, data: Vec<u8>) -> Result<(), SendError> {
        let ack_id = self.take_ackid();
        match frame::encode_payload_frame(self.tube_id, Some(ack_id), data) {
            Ok(frame_data) => {
                let sendack_ctx = Arc::new(Mutex::new(SendAckFutureContext {
                    ack_received: false,
                }));
                if let Err(_) = self.sendacks.try_insert(ack_id, sendack_ctx.clone()) {
                    return Err(SendError::AckIdAlreadyInUseInternalError)
                }

                {
                    let mut sender = self.sender.lock().await;
                    match sender.send_data(frame_data.into()).await {
                        Ok(_) => (),
                        Err(e) => {
                            self.sendacks.remove(&ack_id);
                            return Err(SendError::TransportError(e))
                        },
                    }
                };

                SendAckFuture {
                    ctx: sendack_ctx.clone(),
                }.await;
                self.sendacks.remove(&ack_id);

                Ok(())
            },
            Err(e) => Err(SendError::FrameEncodeError(e)),
        }
    }

    pub async fn send_and_forget(&mut self, data: Vec<u8>) -> Result<(), SendError> {
        match frame::encode_payload_frame(self.tube_id, None, data) {
            Ok(frame_data) => {
                let mut sender = self.sender.lock().await;
                match sender.send_data(frame_data.into()).await {
                    Ok(_) => Ok(()),
                    Err(e) => Err(SendError::TransportError(e)),
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
        let mut tube_mgr = self.tube_manager.lock().unwrap();
        tube_mgr.waker = Some(cx.waker().clone());

        if tube_mgr.terminated {
            return futures::task::Poll::Ready(None);
        }

        match tube_mgr.pending_events.pop_front() {
            Some(tube_event) => match tube_mgr.state_machine.transition_to(&tube_event) {
                tube_event::StateMachineTransitionResult::Valid => {
                    futures::task::Poll::Ready(Some(tube_event))
                },
                tube_event::StateMachineTransitionResult::Invalid(from, to) => {
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
        let mut tube_mgr = self.tube_manager.lock().unwrap();
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
