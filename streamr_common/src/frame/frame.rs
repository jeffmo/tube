use std::collections::HashMap;

pub(in crate::frame) const CLIENT_HAS_FINISHED_SENDING_FRAMETYPE: u8 = 0x0;
pub(in crate::frame) const DRAIN_FRAMETYPE: u8 = 0x1;
pub(in crate::frame) const ESTABLISH_STREAMR_FRAMETYPE: u8 = 0x2;
pub(in crate::frame) const PAYLOAD_FRAMETYPE: u8 = 0x3;
pub(in crate::frame) const PAYLOAD_ACK_FRAMETYPE: u8 = 0x4;
pub(in crate::frame) const SERVER_HAS_FINISHED_SENDING_FRAMETYPE: u8 = 0x5;

/**
 * Each encoded Streamr frame specifies its own structure, but all frames begin 
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

#[derive(Debug, PartialEq)]
pub enum Frame {
    /**
     * This frame is sent by the client when it will send no further Payload 
     * frames for a given Streamr.
     *
     *   +------------------+
     *   |  StreamrId(u16)  |
     *   +------------------+
     */
    ClientHasFinishedSending {
      streamr_id: u16,
    },

    /**
     * This frame is sent to both peers as a signal that a StreamrTransport 
     * needs to be drained (AKA gracefully shutdown). It is up to the 
     * application running on each peer to use this signal to coordinate the 
     * graceful shutdown of all Streamrs hosted by the StreamrTransport this 
     * frame arrived on.
     */
    Drain,

    /**
     * This frame is sent by either peer to indicate the creation of a new 
     * Streamr. Client-generated Streamrs always use an odd-numbered id, and 
     * Server-generated Streamrs always use an even-numbered id.
     *
     *   +------------------+-----------------------------+
     *   |  StreamrId(u16)  |  Utf8EncodedJSONHeaders(*)  |
     *   +------------------+-----------------------------+
     */
    EstablishStreamr {
      streamr_id: u16,
      headers: HashMap<String, String>,
    },

    /**
     * This frame is sent by either peer to transmit data.
     *
     *   +------------------+-------------------+-------------+-----------+
     *   |  StreamrId(u16)  |  AckRequested(1)  |  AckId(15)  |  Data(*)  |
     *   +------------------+-------------------+-------------+-----------+
     */
    Payload {
      streamr_id: u16,
      ack_id: Option<u16>,
      data: Vec<u8>,
    },

    /**
     * This frame is sent by either peer when it receives a Payload frame that 
     * specifies an ack_id. Note that receipt of a PayloadAck frame only means 
     * the other peer received a Payload, it does not necessarily mean that the 
     * application successfully processed the payload.
     *
     *   +------------------+---------------+-------------+
     *   |  StreamrId(u16)  |  RESERVED(1)  |  AckId(15)  |
     *   +------------------+---------------+-------------+
     */
    PayloadAck {
      streamr_id: u16,
      ack_id: u16,
    },

    /**
     * This frame is sent by the server when it will send no further Payload 
     * frames for a given Streamr.
     *
     *   +------------------+
     *   |  StreamrId(u16)  |
     *   +------------------+
     */
    ServerHasFinishedSending {
      streamr_id: u16,
    },
}
