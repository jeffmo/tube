use std::collections::HashMap;

pub enum Frame {
    /**
     * This frame is sent by the client when it will send no further Payload 
     * frames for a given Streamr.
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
     */
    EstablishStreamr {
      streamr_id: u16,
      headers: HashMap<String, String>,
    },

    /**
     * This frame is sent by either peer to transmit data.
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
     */
    PayloadAck {
      streamr_id: u16,
      ack_id: u16,
    },

    /**
     * This frame is sent by the server when it will send no further Payload 
     * frames for a given Streamr.
     */
    ServerHasFinishedSending {
      streamr_id: u16,
    },
}
