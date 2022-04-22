use std::collections::HashMap;

use serde_json;

use super::frame;

#[derive(Debug)]
pub enum FrameEncodeError {
    AckIdTooLarge(u16),
    DataTooLarge(usize),
    HeaderJsonEncodeError(serde_json::error::Error),
}

pub fn encode_client_has_finished_sending_frame(
    streamr_id: u16,
) -> Result<Vec<u8>, FrameEncodeError> {
    let streamrid_bytes = streamr_id.to_be_bytes();
    Ok(vec![
        frame::CLIENT_HAS_FINISHED_SENDING_FRAMETYPE, 
        0, 2,
        streamrid_bytes[0], 
        streamrid_bytes[1],
    ])
}

pub fn encode_drain_frame() -> Result<Vec<u8>, FrameEncodeError> {
    Ok(vec![
        frame::DRAIN_FRAMETYPE,
        // bodylen=0
        0, 0
    ])
}

pub fn encode_establish_streamr_frame(
    streamr_id: u16, 
    headers: HashMap<String, String>
) -> Result<Vec<u8>, FrameEncodeError> {
    let streamrid_bytes = streamr_id.to_be_bytes();
    let mut headers_json_str_bytes = match serde_json::to_string(&headers) {
        Ok(json_str) => json_str.into_bytes(),
        Err(json_err) => return Err(FrameEncodeError::HeaderJsonEncodeError(json_err))
    };
    let body_len: u16 = 2 + (headers_json_str_bytes.len() as u16);
    let body_len_bytes = body_len.to_be_bytes();
    let mut bytes = vec![
        frame::ESTABLISH_STREAMR_FRAMETYPE, 
        body_len_bytes[0],
        body_len_bytes[1],
        streamrid_bytes[0], 
        streamrid_bytes[1],
    ];
    bytes.append(&mut headers_json_str_bytes);
    Ok(bytes)
}

pub fn encode_payload_frame(
    streamr_id: u16,
    ack_id: Option<u16>,
    mut data: Vec<u8>,
) -> Result<Vec<u8>, FrameEncodeError> {
    // BodyLenBytes maxes out at 2^16, so ensure that the size of data fits into
    // that limit
    if data.len() > 2 + 2 + (2^16) {
        return Err(FrameEncodeError::DataTooLarge(data.len()))
    }

    let streamrid_bytes = streamr_id.to_be_bytes();
    let ack_bytes = match ack_id {
        Some(ack_id) => {
            if ((0b1000_0000 << 8) & ack_id) > 0 {
                return Err(FrameEncodeError::AckIdTooLarge(ack_id));
            } else {
                ((0b1000_0000 << 8) | ack_id).to_be_bytes()
            }
        },
        None => [0, 0]
    };
    let body_len = 2 + 2 + (data.len() as u16);
    let body_len_bytes = body_len.to_be_bytes();
    let mut bytes = vec![
        frame::PAYLOAD_FRAMETYPE,
        body_len_bytes[0],
        body_len_bytes[1],
        streamrid_bytes[0],
        streamrid_bytes[1],
        ack_bytes[0],
        ack_bytes[1],
    ];
    bytes.append(&mut data);
    Ok(bytes)
}

pub fn encode_payload_ack_frame(
    streamr_id: u16,
    ack_id: u16,
) -> Result<Vec<u8>, FrameEncodeError> {
    let streamrid_bytes = streamr_id.to_be_bytes();
    let ack_id_bytes = if ((0b1000_0000 << 8) & ack_id) > 0 {
        return Err(FrameEncodeError::AckIdTooLarge(ack_id));
    } else {
        ack_id.to_be_bytes()
    };
    Ok(vec![
       frame::PAYLOAD_ACK_FRAMETYPE,
       0, 4,
       streamrid_bytes[0],
       streamrid_bytes[1],
       ack_id_bytes[0],
       ack_id_bytes[1],
    ])
}

pub fn encode_server_has_finished_sending_frame(
    streamr_id: u16,
) -> Result<Vec<u8>, FrameEncodeError> {
    let streamrid_bytes = streamr_id.to_be_bytes();
    Ok(vec![
        frame::SERVER_HAS_FINISHED_SENDING_FRAMETYPE, 
        0, 2,
        streamrid_bytes[0], 
        streamrid_bytes[1],
    ])
}

