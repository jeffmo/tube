use futures;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use crate::common::frame;
use crate::common::InvertedFuture;
use crate::common::PeerType;
use crate::common::UniqueId;
use crate::common::UniqueIdError;
use crate::common::UniqueIdManager;
use super::TubeEvent;
use super::TubeEvent_StreamError;
use super::TubeEventTag;
use super::tube_manager::TubeCompletionState;
use super::tube_manager::TubeManager;

pub mod error {
    use super::Duration;
    use super::frame;

    #[derive(Debug)]
    pub enum AbortError {
        AlreadyAborted(frame::AbortReason),
        AlreadyClosed,
        FrameEncodeError(frame::encode::FrameEncodeError),
        FatalTransportError(hyper::Error),
    }

    #[derive(Debug)]
    pub enum HasFinishedSendingError {
        AlreadyMarkedAsFinishedSending,
        FrameEncodeError(frame::encode::FrameEncodeError),
        InternalError(String),
        FatalTransportError(hyper::Error),
        TubeAlreadyAborted(frame::AbortReason),
    }

    #[derive(Debug)]
    pub enum SendError {
        AckIdAlreadyInUseInternalError,
        AckIdsExhausted,
        FrameEncodeError(frame::encode::FrameEncodeError),
        TimedOutWaitingOnAck(Duration),
        TransportError(hyper::Error),
        UnknownTransportError,
    }
}

async fn send_abort(
    tube_id: u16,
    reason: frame::AbortReason,
    tube_manager: &Arc<Mutex<TubeManager>>,
    sender: &Arc<tokio::sync::Mutex<hyper::body::Sender>>,
) -> Result<(), error::AbortError> {
    let frame_data = match frame::encode::abort_frame(tube_id, reason.clone()) {
        Ok(frame_data) => frame_data,
        Err(e) => return Err(error::AbortError::FrameEncodeError(e)),
    };

    {
        let mut tube_mgr = tube_manager.lock().unwrap();
        match &mut tube_mgr.completion_state {
            TubeCompletionState::AbortedFromLocal(reason) | 
                TubeCompletionState::AbortedFromRemote(reason) => 
                return Err(error::AbortError::AlreadyAborted(reason.clone())),

            TubeCompletionState::Closed => 
                return Err(error::AbortError::AlreadyClosed),
                
            _ => (),
        };

        tube_mgr.completion_state = TubeCompletionState::AbortedFromLocal(reason);
    };

    // TODO: Stick a timeout on these awaits so that some kind of pathological 
    //       hyper issue doesn't block the tube_mgr Mutex forever or something
    let mut sender = sender.lock().await;
    match sender.send_data(frame_data.into()).await {
        Ok(_) => Ok(()),
        // TODO: Should this just be a panic? If we get into this state we don't
        //       really know if the client and server are synchronized on the 
        //       state of this Tube...havoc?
        Err(e) => Err(error::AbortError::FatalTransportError(e)),
    }

    // TODO: !!! Need to somehow remove this tube from the channel's map of 
    //           TubeManagers! As-is, we only remove from that map if we recieve and Abort or a
    //           closing HasFinishedSending from the peer...but not if we initiate full closure
    //           from the local Tube object!!
}

async fn send_has_finished_sending(
    peer_type: PeerType,
    tube_id: u16,
    tube_manager: &Arc<Mutex<TubeManager>>,
    sender: &Arc<tokio::sync::Mutex<hyper::body::Sender>>,
) -> Result<(), error::HasFinishedSendingError> {
    let maybe_frame_data = match peer_type {
        PeerType::Client => 
            frame::encode::client_has_finished_sending_frame(tube_id),
        PeerType::Server => 
            frame::encode::server_has_finished_sending_frame(tube_id),
    };
    let frame_data = match maybe_frame_data {
        Ok(data) => data,
        Err(e) => return Err(error::HasFinishedSendingError::FrameEncodeError(e)),
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
                return Err(error::HasFinishedSendingError::AlreadyMarkedAsFinishedSending),
            (&AbortedFromLocal(ref reason), _) |
                (&AbortedFromRemote(ref reason), _) =>
                return Err(error::HasFinishedSendingError::TubeAlreadyAborted(reason.clone())),
        };

        tube_mgr.completion_state = new_state;
    };

    // TODO: Stick a timeout on these awaits so that some kind of pathological 
    //       hyper issue doesn't block the tube_mgr Mutex forever or something
    let transport_error = {
        let mut sender = sender.lock().await;
        sender.send_data(frame_data.into()).await
    };

    // If the transmit failed, we can't be certain if the HasFinishedSending was
    // actually received by the peer...so [try to] abort the Tube before 
    // returning a transport error.
    if let Err(e) = transport_error {
        let _ = send_abort(
            tube_id, 
            frame::AbortReason::TransportErrorWhileSynchronizingTubeState,
            tube_manager,
            sender,
        ).await;

        // At this point the Tube is considered terminal in an Aborted state and
        // cannot send or receive data. The application must be replace it with
        // a new Tube.
        return Err(error::HasFinishedSendingError::FatalTransportError(e));
    }

    // TODO: !!! Need to somehow remove this tube from the channel's map of 
    //           TubeManagers if it is now fully Closed! As-is, we only remove 
    //           from that map if we recieve and Abort or a closing 
    //           HasFinishedSending from the peer...but not if we initiate full
    //           closure from the local Tube object!!

    Ok(())
}

#[derive(Debug)]
pub struct Tube {
    ackid_manager: UniqueIdManager,
    last_tube_event: Option<TubeEventTag>,
    sender: Arc<tokio::sync::Mutex<hyper::body::Sender>>,
    tube_id: UniqueId,
    tube_manager: Arc<Mutex<TubeManager>>,
    peer_type: PeerType,
}
impl Tube {
    pub async fn abort(&mut self, ) -> Result<(), error::AbortError> {
        self.abort_internal(frame::AbortReason::ApplicationAbort).await
    }
    
    pub(in crate) async fn abort_internal(
        &mut self, 
        reason: frame::AbortReason,
    ) -> Result<(), error::AbortError> {
        send_abort(
            self.tube_id.val(), 
            reason, 
            &self.tube_manager,
            &self.sender,
        ).await
    }

    pub fn get_id(&self) -> u16 {
        return self.tube_id.val();
    }

    pub async fn has_finished_sending(&self) -> Result<(), error::HasFinishedSendingError> {
        send_has_finished_sending(
            self.peer_type,
            self.tube_id.val(),
            &self.tube_manager,
            &self.sender,
        ).await
    }

    pub(in crate) fn new(
        peer_type: PeerType,
        tube_id: UniqueId,
        sender: Arc<tokio::sync::Mutex<hyper::body::Sender>>, 
        tube_manager: Arc<Mutex<TubeManager>>,
    ) -> Self {
        Tube {
            ackid_manager: UniqueIdManager::new(),
            last_tube_event: None,
            sender,
            tube_id,
            tube_manager,
            peer_type,
        }
    }

    pub async fn send(
        &mut self, 
        data: Vec<u8>,
        ack_timeout: Duration,
    ) -> Result<(), error::SendError> {
        let ack_id = match self.ackid_manager.take_id() {
            Ok(ack_id) => ack_id,
            Err(UniqueIdError::NoIdsAvailable) => return Err(error::SendError::AckIdsExhausted),
        };

        let frame_data = match frame::encode::payload_frame(
            self.tube_id.val(), 
            Some(ack_id.val()), 
            data,
        ) {
            Ok(frame_data) => frame_data,
            Err(e) => return Err(error::SendError::FrameEncodeError(e)),
        };

        let (sendack_future, sendack_resolver) = InvertedFuture::<()>::new();
        {
            let mut tube_mgr = self.tube_manager.lock().unwrap();
            if let Err(_) = tube_mgr.sendacks.try_insert(ack_id.val(), sendack_resolver) {
                return Err(error::SendError::AckIdAlreadyInUseInternalError)
            }
        }

        {
            let mut sender = self.sender.lock().await;
            if let Err(e) = sender.send_data(frame_data.into()).await {
                let mut tube_mgr = self.tube_manager.lock().unwrap();
                tube_mgr.sendacks.remove(&ack_id.val());
                return Err(error::SendError::TransportError(e))
            }
        };

        let sendack_future_with_timeout = 
            tokio::time::timeout(ack_timeout, sendack_future);
        let sendack_future_result = sendack_future_with_timeout.await;

        {
            let mut tube_mgr = self.tube_manager.lock().unwrap();
            tube_mgr.sendacks.remove(&ack_id.val());
        }

        if let Err(_) = sendack_future_result {
            return Err(error::SendError::TimedOutWaitingOnAck(ack_timeout));
        }

        Ok(())

    }

    pub async fn send_and_forget(&mut self, data: Vec<u8>) -> Result<(), error::SendError> {
        match frame::encode::payload_frame(self.tube_id.val(), None, data) {
            Ok(frame_data) => {
                let mut sender = self.sender.lock().await;
                if let Err(e) = sender.send_data(frame_data.into()).await {
                    return Err(error::SendError::TransportError(e));
                }
                Ok(())
            },
            Err(e) => Err(error::SendError::FrameEncodeError(e)),
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

        match (self.last_tube_event.as_ref(), tube_mgr.pending_events.pop_front()) {
            // No more pending_events
            (_, None) => {
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
            },

            // TODO: Enumerate various TubeEvents and validate state transitions 
            //       here. Issue a 
            //       TubeEvent::StreamError(InvalidTubeEventTransition) when the
            //       transition doesn't make sense.
            (_, Some(tube_event)) => 
                futures::task::Poll::Ready(Some(tube_event)),
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
                let tube_id = self.tube_id.take();
                let tube_manager = self.tube_manager.clone();
                let sender = self.sender.clone();
                tokio::spawn(async move {
                    if let Err(e) = send_has_finished_sending(
                        peer_type,
                        tube_id.val(),
                        &tube_manager,
                        &sender,
                    ).await {
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

                let tube_id = self.tube_id.take();
                let tube_manager = self.tube_manager.clone();
                let sender = self.sender.clone();
                tokio::spawn(async move {
                    if let Err(e) = send_abort(
                        tube_id.val(), 
                        frame::AbortReason::ApplicationError,
                        &tube_manager,
                        &sender,
                    ).await {
                        // TODO: Should this just be a panic? If we get into 
                        //       this state we don't really know if the client 
                        //       and server are synchronized on the state of 
                        //       this Tube...havoc?
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
mod tube_tests {
    use super::*;

    use crate::common::InvertedFuture;
    use crate::tube;

    struct TestTubeStuff {
        req_body: hyper::body::Body,
        tube_manager: Arc<Mutex<TubeManager>>,
    }

    fn make_test_tube() -> (Tube, TestTubeStuff) {
        let (body_sender, req_body) = hyper::Body::channel();
        let body_sender = Arc::new(tokio::sync::Mutex::new(body_sender));
        let mut id_manager = UniqueIdManager::new();
        let tube_id = id_manager.take_id().unwrap();
        let tube_manager = Arc::new(Mutex::new(TubeManager::new()));
        let tube = Tube::new(
            PeerType::Client,
            tube_id,
            body_sender,
            tube_manager.clone(),
        );

        (tube, TestTubeStuff {
            req_body,
            tube_manager,
        })
    }

    #[tokio::test]
    async fn send_errors_if_ackid_already_in_use() {
        let (mut tube, tube_stuff) = make_test_tube();

        // Add a sendack(id=0) to the TubeManager
        let (_fut, res) = InvertedFuture::<()>::new();
        {
            let mut tube_mgr = tube_stuff.tube_manager.lock().unwrap();
            tube_mgr.sendacks.insert(0, res);
        }

        match tube.send("test data".into(), Duration::from_millis(100)).await {
            Err(tube::error::SendError::AckIdAlreadyInUseInternalError) => {
                let tube_mgr = tube_stuff.tube_manager.lock().unwrap();
                assert_eq!(tube_mgr.sendacks.len(), 1);
                assert!(tube_mgr.sendacks.contains_key(&0));
            },

            unexpected => assert!(
                false, 
                "Unexpected result from Tube::Send(): {:?}", 
                unexpected,
            )
        }
    }

    #[tokio::test]
    async fn send_errors_if_ack_not_received_in_time() {
        let (mut tube, tube_stuff) = make_test_tube();
        let timeout = Duration::from_nanos(1);
        match tube.send("test data".into(), timeout.clone()).await {
            Err(tube::error::SendError::TimedOutWaitingOnAck(err_timeout)) => {
                assert_eq!(err_timeout, timeout);

                // There should be no SendAck entry left polluting the TubeManager
                let tube_mgr = tube_stuff.tube_manager.lock().unwrap();
                assert_eq!(tube_mgr.sendacks.len(), 0);
            },

            unexpected => assert!(
                false,
                "Unexpected result from Tube::send(): {:?}",
                unexpected,
            ),
        }
    }
/*
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
*/
}
