use std::collections::HashMap;

pub(in super) const CLIENT_HAS_FINISHED_SENDING_FRAMETYPE: u8 = 0x0;
pub(in super) const DRAIN_FRAMETYPE: u8 = 0x1;
pub(in super) const NEWTUBE_FRAMETYPE: u8 = 0x2;
pub(in super) const PAYLOAD_FRAMETYPE: u8 = 0x3;
pub(in super) const PAYLOAD_ACK_FRAMETYPE: u8 = 0x4;
pub(in super) const SERVER_HAS_FINISHED_SENDING_FRAMETYPE: u8 = 0x5;
pub(in super) const ABORT_FRAMETYPE: u8 = 0x6;

/**
 * Each encoded Tube frame specifies its own structure, but all frames begin 
 * with the following header structure:
 *
 *   +-----------------+----------------------------+
 *   |  FrameType(u8)  |  FrameBodyByteLength(u16)  |
 *   +-----------------+----------------------------+
 *
 * The FrameType value indicates how the frame's body should be parsed, and the 
 * FrameBodyByteLength value indicates the number of bytes the frame's body 
 * occupies. 
 *
 * FrameBodyByteLength only specifies the size of the frame's body, it does not 
 * account for the 3 bytes used in the frame's header structure.
 */
#[derive(Clone,Debug,PartialEq)]
pub enum AbortReason {
    ApplicationAbort,
    ApplicationError,
    TransportErrorWhileSynchronizingTubeState,
    Unknown,
}
impl From<u8> for AbortReason {
    fn from(reason: u8) -> Self {
        match reason {
            0x0 => AbortReason::ApplicationAbort,
            0x1 => AbortReason::ApplicationError,
            0x2 => AbortReason::TransportErrorWhileSynchronizingTubeState,
            _   => AbortReason::Unknown,
        }
    }
}
impl Into<u8> for AbortReason {
    fn into(self) -> u8 {
        match self {
            AbortReason::ApplicationAbort                          => 0x00,
            AbortReason::ApplicationError                          => 0x01,
            AbortReason::TransportErrorWhileSynchronizingTubeState => 0x02,
            AbortReason::Unknown                                   => 0xFF,
        }
    }
}

#[derive(Clone,Debug,PartialEq)]
pub enum Frame {
    /**
     * This frame is sent by the client when it will send no further Payload 
     * frames for a given Tube.
     *
     *   +---------------+
     *   |  TubeId(u16)  |
     *   +---------------+
     */
    ClientHasFinishedSending {
        tube_id: u16,
    },

    /**
     * This frame is sent to both peers as a signal that a TubeTransport 
     * needs to be drained (AKA gracefully shutdown). It is up to the 
     * application running on each peer to use this signal to coordinate the 
     * graceful shutdown of all Tubes hosted by the TubeTransport this 
     * frame arrived on.
     */
    Drain,

    /**
     * This frame is sent by either peer to indicate the creation of a new 
     * Tube. Client-generated Tubes always use an odd-numbered id, and 
     * Server-generated Tubes always use an even-numbered id.
     *
     *   +---------------+-----------------------------+
     *   |  TubeId(u16)  |  Utf8EncodedJSONHeaders(*)  |
     *   +---------------+-----------------------------+
     */
    NewTube {
        tube_id: u16,
        headers: HashMap<String, String>,
    },

    /**
     * This frame is sent by either peer to transmit data.
     *
     *   +---------------+-------------------+-------------+-----------+
     *   |  TubeId(u16)  |  AckRequested(1)  |  AckId(15)  |  Data(*)  |
     *   +---------------+-------------------+-------------+-----------+
     */
    Payload {
        tube_id: u16,
        ack_id: Option<u16>,
        data: Vec<u8>,
    },

    /**
     * This frame is sent by either peer when it receives a Payload frame that 
     * specifies an ack_id. Note that receipt of a PayloadAck frame only means 
     * the other peer received a Payload, it does not necessarily mean that the 
     * application successfully processed the payload.
     *
     *   +---------------+---------------+-------------+
     *   |  TubeId(u16)  |  RESERVED(1)  |  AckId(15)  |
     *   +---------------+---------------+-------------+
     */
    PayloadAck {
        tube_id: u16,
        ack_id: u16,
    },

    /**
     * This frame is sent by the server when it will send no further Payload 
     * frames for a given Tube.
     *
     *   +---------------+
     *   |  TubeId(u16)  |
     *   +---------------+
     */
    ServerHasFinishedSending {
        tube_id: u16,
    },

    /**
     * This frame is sent by either peer in order to immediately end the Tube, 
     * no waiting for both peers to agree by sending their respective 
     * HasFinishedSending frame.
     *
     *   +-----------------------------------+
     *   |  TubeId(u16)  |  AbortReason(u8)  |
     *   +-----------------------------------+
     */
    Abort {
        tube_id: u16,
        reason: AbortReason,
    },
}
