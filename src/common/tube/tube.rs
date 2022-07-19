use futures;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;

use crate::common::frame;
use crate::common::InvertedFuture;
use crate::common::PeerType;
use super::tube_event;
use super::tube_event::TubeEvent;
use super::tube_event::TubeEvent_StreamError;
use super::tube_manager::TubeCompletionState;
use super::tube_manager::TubeManager;

#[derive(Debug)]
pub enum AbortError {
    AlreadyAborted(frame::AbortReason),
    AlreadyClosed,
    FrameEncodeError(frame::FrameEncodeError),
    TransportError(hyper::Error),
}

#[derive(Debug)]
pub enum HasFinishedSendingError {
    AlreadyMarkedAsFinishedSending,
    FrameEncodeError(frame::FrameEncodeError),
    InternalError(String),
    FatalTransportError(hyper::Error),
    TubeAlreadyAborted(frame::AbortReason),
}

#[derive(Debug)]
pub enum SendError {
    AckIdAlreadyInUseInternalError,
    FrameEncodeError(frame::FrameEncodeError),
    TransportError(hyper::Error),
    UnknownTransportError,
}

async fn send_abort(
    tube_id: u16,
    reason: frame::AbortReason,
    tube_manager: &Arc<Mutex<TubeManager>>,
    sender: &Arc<tokio::sync::Mutex<hyper::body::Sender>>,
) -> Result<(), AbortError> {
    let frame_data = match frame::encode_abort_frame(tube_id, reason.clone()) {
        Ok(frame_data) => frame_data,
        Err(e) => return Err(AbortError::FrameEncodeError(e)),
    };

    {
        let mut tube_mgr = tube_manager.lock().unwrap();
        match &mut tube_mgr.completion_state {
            TubeCompletionState::AbortedFromLocal(reason) | 
                TubeCompletionState::AbortedFromRemote(reason) => 
                return Err(AbortError::AlreadyAborted(reason.clone())),

            TubeCompletionState::Closed => 
                return Err(AbortError::AlreadyClosed),
                
            _ => (),
        };

        tube_mgr.completion_state = TubeCompletionState::AbortedFromLocal(reason);
    };

    // TODO: Stick a timeout on these awaits so that some kind of pathological 
    //       hyper issue doesn't block the tube_mgr Mutex forever or something
    let mut sender = sender.lock().await;
    match sender.send_data(frame_data.into()).await {
        Ok(_) => Ok(()),
        Err(e) => Err(AbortError::TransportError(e)),
    }
}

async fn send_has_finished_sending(
    peer_type: PeerType,
    tube_id: u16,
    tube_manager: &Arc<Mutex<TubeManager>>,
    sender: &Arc<tokio::sync::Mutex<hyper::body::Sender>>,
) -> Result<(), HasFinishedSendingError> {
    let maybe_frame_data = match peer_type {
        PeerType::Client => 
            frame::encode_client_has_finished_sending_frame(tube_id),
        PeerType::Server => 
            frame::encode_server_has_finished_sending_frame(tube_id),
    };
    let frame_data = match maybe_frame_data {
        Ok(data) => data,
        Err(e) => return Err(HasFinishedSendingError::FrameEncodeError(e)),
    };

    {
        let mut tube_mgr = tube_manager.lock().unwrap();
        use TubeCompletionState::*;
        use PeerType::*;
        let new_state = match (&tube_mgr.completion_state, &peer_type) {
            (&Open, &Client) => 
                ClientHasFinishedSending,
            (&Open, &Server) => 
                ServerHasFinishedSending,
            (&ClientHasFinishedSending, &Server) | (&ServerHasFinishedSending, &Client) =>
                Closed,
            (&ClientHasFinishedSending, &Client) | 
                (&ServerHasFinishedSending, &Server) |
                (&Closed, _) =>
                return Err(HasFinishedSendingError::AlreadyMarkedAsFinishedSending),
            (&AbortedFromLocal(ref reason), _) |
                (&AbortedFromRemote(ref reason), _) =>
                return Err(HasFinishedSendingError::TubeAlreadyAborted(reason.clone())),
        };

        tube_mgr.completion_state = new_state;
    };

    // TODO: Stick a timeout on these awaits so that some kind of pathological 
    //       hyper issue doesn't block the tube_mgr Mutex forever or something
    let transport_error = {
        let mut sender = sender.lock().await;
        match sender.send_data(frame_data.into()).await {
            Ok(_) => None,
            Err(e) => Some(e),
        }
    };

    // If the transmit failed, we can't be certain if the HasFinishedSending was
    // actually received by the peer...so [try to] abort the Tube before 
    // returning a transport error.
    if let Some(e) = transport_error {
        let _ = send_abort(
            tube_id, 
            frame::AbortReason::TransportErrorWhileSynchronizingTubeState,
            tube_manager,
            sender,
        ).await;

        // At this point the Tube is considered terminal in an Aborted state and
        // cannot send or receive data. The application must be replace it with
        // a new Tube.
        return Err(HasFinishedSendingError::FatalTransportError(e));
    }

    Ok(())
}

#[derive(Debug)]
pub struct Tube {
    available_ackids: VecDeque<u16>,
    ack_id_counter: u16,
    sender: Arc<tokio::sync::Mutex<hyper::body::Sender>>,
    tube_id: u16,
    tube_manager: Arc<Mutex<TubeManager>>,
    peer_type: PeerType,
}
impl Tube {
    pub async fn abort(&mut self, ) -> Result<(), AbortError> {
        self.abort_internal(frame::AbortReason::ApplicationAbort).await
    }
    
    pub(in crate) async fn abort_internal(
        &mut self, 
        reason: frame::AbortReason,
    ) -> Result<(), AbortError> {
        send_abort(
            self.tube_id, 
            reason, 
            &self.tube_manager,
            &self.sender,
        ).await
    }

    pub fn get_id(&self) -> u16 {
        return self.tube_id;
    }

    pub async fn has_finished_sending(&self) -> Result<(), HasFinishedSendingError> {
        send_has_finished_sending(
            self.peer_type,
            self.tube_id,
            &self.tube_manager,
            &self.sender,
        ).await
    }

    pub(in crate) fn new(
        peer_type: PeerType,
        tube_id: u16,
        sender: Arc<tokio::sync::Mutex<hyper::body::Sender>>, 
        tube_manager: Arc<Mutex<TubeManager>>,
    ) -> Self {
        Tube {
            ack_id_counter: 0,
            available_ackids: VecDeque::new(),
            sender,
            tube_id,
            tube_manager,
            peer_type,
        }
    }

    pub async fn send(&mut self, data: Vec<u8>) -> Result<(), SendError> {
        let ack_id = self.take_ackid();
        match frame::encode_payload_frame(self.tube_id, Some(ack_id), data) {
            Ok(frame_data) => {
                let (sendack_future, sendack_resolver) = InvertedFuture::<()>::new();
                {
                    let mut tube_mgr = self.tube_manager.lock().unwrap();
                    if let Err(_) = tube_mgr.sendacks.try_insert(ack_id, sendack_resolver) {
                        return Err(SendError::AckIdAlreadyInUseInternalError)
                    }
                }

                {
                    let mut sender = self.sender.lock().await;
                    match sender.send_data(frame_data.into()).await {
                        Ok(_) => (),
                        Err(e) => {
                            {
                                let mut tube_mgr = self.tube_manager.lock().unwrap();
                                tube_mgr.sendacks.remove(&ack_id);
                            }
                            return Err(SendError::TransportError(e))
                        },
                    }
                };

                // TODO: Await this, but with some kind of a timeout...
                sendack_future.await;

                {
                    let mut tube_mgr = self.tube_manager.lock().unwrap();
                    tube_mgr.sendacks.remove(&ack_id);
                }

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
}
impl futures::stream::Stream for Tube {
    type Item = TubeEvent;

    fn poll_next(
        self: core::pin::Pin<&mut Self>,
        cx: &mut futures::task::Context,
    ) -> futures::task::Poll<Option<Self::Item>> {
        let mut tube_mgr = self.tube_manager.lock().unwrap();
        tube_mgr.waker = Some(cx.waker().clone());

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

            None => {
                use TubeCompletionState::*;
                match (&self.peer_type, &tube_mgr.completion_state) {
                    (_, AbortedFromLocal(_)) |
                        (_, AbortedFromRemote(_)) => {
                        // TODO: Error all pending SendAcks
                        futures::task::Poll::Ready(None)
                    },

                    (&PeerType::Client, &Open | &ClientHasFinishedSending) |
                    (&PeerType::Server, &Open | &ServerHasFinishedSending) => 
                        futures::task::Poll::Pending,

                    (&PeerType::Client, &Closed | &ServerHasFinishedSending) |
                    (&PeerType::Server, &Closed | &ClientHasFinishedSending) =>
                        futures::task::Poll::Ready(None),
                }
            }
        }
    }
}
impl Drop for Tube {
    fn drop(&mut self) {
        let completion_state = {
            let tube_mgr = self.tube_manager.lock().unwrap();
            tube_mgr.completion_state.clone()
        };
        let remote_peer_str = match self.peer_type {
            PeerType::Client => "server",
            PeerType::Server => "client"
        };

        let peer_not_finished_str = format!(
            "Dropping tube(id={}) before {} has finished sending!",
            self.tube_id,
            remote_peer_str,
        );

        use PeerType::*;
        use TubeCompletionState::*;
        log::trace!(
            "Checking completion_state={:?} before dropping Tube(id={})...", 
            &completion_state, 
            self.tube_id,
        );
        match (self.peer_type, &completion_state) {
            (_, &AbortedFromLocal(_) | &AbortedFromRemote(_) | &Closed) => (),

            (Client, &ServerHasFinishedSending) |
            (Server, &ClientHasFinishedSending) => {
                let peer_type = self.peer_type;
                let tube_id = self.tube_id;
                let tube_manager = self.tube_manager.clone();
                let sender = self.sender.clone();
                tokio::spawn(async move {
                    if let Err(e) = send_has_finished_sending(
                        peer_type,
                        tube_id,
                        &tube_manager,
                        &sender,
                    ).await {
                        let remote_peer = match peer_type {
                            PeerType::Client => "server",
                            PeerType::Server => "client"
                        };
                        log::error!(
                            "Attempted to communicate to the {:?} that \
                             Tube(id={}) has finished sending when dropping \
                             the Tube object, but failed: {:?}", 
                            remote_peer_str, 
                            tube_id, 
                            e
                        )
                    }
                });
            },

            (Client, &ClientHasFinishedSending) |
            (Server, &ServerHasFinishedSending) |
            (_, &Open) => {
                log::error!(
                    "Dropping Tube(id={}) before {} has finished sending! \
                     Sending abort to {}",
                    self.tube_id,
                    remote_peer_str,
                    remote_peer_str,
                );

                let tube_id = self.tube_id;
                let tube_manager = self.tube_manager.clone();
                let sender = self.sender.clone();
                tokio::spawn(async move {
                    if let Err(e) = send_abort(
                        tube_id, 
                        frame::AbortReason::ApplicationError,
                        &tube_manager,
                        &sender,
                    ).await {
                        log::error!(
                            "Attempted to send an Abort for Tube(id={}) \
                             to the {}, but failed: {:?}", 
                            remote_peer_str,
                            tube_id, 
                            e
                        )
                    }
                });
            },
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

/*
#[cfg(test)]
mod tube_tests {
    use futures::StreamExt;
    use hyper;
    use tokio;

    use crate::tube::TubeEvent;
    use crate::tube::TubeEvent_StreamError;
    use crate::tube::TubeEventTag;
    use crate::tube::Tube;

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
*/
