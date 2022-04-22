use std::collections::HashMap;
use std::collections::VecDeque;

use serde_json;

use super::frame;

// Returned by Decoder::decode() and provides context around 
// a FrameParseError
#[allow(dead_code)]
#[derive(Debug)]
pub struct FrameDecodeError {
    // TODO: Remove these allow(dead_code) tags if they aren't needed anymore
    #[allow(dead_code)]
    pub parse_error: FrameParseError,
    #[allow(dead_code)]
    pub num_frames_parsed_successfully: usize,
}

#[derive(Debug)]
pub enum FrameParseError {
    InternalByteOffsetLogicError(String),
    HeaderJsonDecodeError(serde_json::error::Error),
    HeaderUtf8Error(std::str::Utf8Error),
    UnknownFrameType(u8),
}

fn double_u8_to_u16(left_byte: u8, right_byte: u8) -> u16 {
    // 1) LLLLLLLL -> 00000000LLLLLLLL
    // 2) 00000000LLLLLLLL -> LLLLLLLL00000000
    // 3) LLLLLLLL00000000 -> LLLLLLLLRRRRRRRR
    ((left_byte as u16) << 8) | (right_byte as u16)
}

fn parse_frame_body(frame_type: u8, mut frame_body_data: VecDeque<u8>) 
        -> Result<frame::Frame, FrameParseError> {
    match frame_type {
        // ClientHasFinishedSending
        frame::CLIENT_HAS_FINISHED_SENDING_FRAMETYPE => {
            let streamr_id = double_u8_to_u16(
                frame_body_data[0],
                frame_body_data[1],
            );
            Ok(frame::Frame::ClientHasFinishedSending { streamr_id })
        },

        // Drain
        1 => {
            Ok(frame::Frame::Drain)
        },

        // EstablishStreamr
        2 => {
            let mut header_bytes = frame_body_data.split_off(2);
            let streamr_id = double_u8_to_u16(
                frame_body_data[0],
                frame_body_data[1],
            );
            let headers_str = match std::str::from_utf8(header_bytes.make_contiguous()) {
                Ok(str) => str,
                Err(utf8_err) => return Err(FrameParseError::HeaderUtf8Error(utf8_err))
            };
            let headers = match serde_json::from_str::<HashMap<String, String>>(&headers_str) {
                Ok(headers) => headers,
                Err(json_err) => return Err(FrameParseError::HeaderJsonDecodeError(json_err))
            };
            Ok(frame::Frame::EstablishStreamr { streamr_id, headers })
        },

        // Payload
        3 => {
            let data = frame_body_data.split_off(4).make_contiguous().to_vec();
            let streamr_id = double_u8_to_u16(
                frame_body_data[0],
                frame_body_data[1],
            );
            let ack_id = 
                // First bit of ack_id indicates whether an ACK is expected for 
                // this payload and should not be considered when interpreting 
                // the ack_id value.
                if (0b1000_0000 & frame_body_data[2]) > 0 {
                    Some(double_u8_to_u16(
                        0b0111_1111 & frame_body_data[2],
                        frame_body_data[3]
                    ))
                } else {
                    None
                };
            Ok(frame::Frame::Payload { streamr_id, ack_id, data })
        },

        // PayloadAck
        4 => {
            let streamr_id = double_u8_to_u16(
                frame_body_data[0],
                frame_body_data[1],
            );
            let ack_id = double_u8_to_u16(
                // ack_ids are always 15 bits. The 16th/MSB here is only used in
                // Payload frames to indicate if an ack is actually requested.
                //
                // For now we zero-out this bit in PayloadAck frames to reserve
                // this bit for other things in the future (e.g. a feature to 
                // enable an ack to indicate "received" vs "processed", etc).
                127 & frame_body_data[2],
                frame_body_data[3],
            );
            Ok(frame::Frame::PayloadAck { streamr_id, ack_id })
        },

        // ServerHasFinishedSending
        5 => {
            let streamr_id = double_u8_to_u16(
                frame_body_data[0],
                frame_body_data[1],
            );
            Ok(frame::Frame::ServerHasFinishedSending { streamr_id })
        },

        _ => Err(FrameParseError::UnknownFrameType(frame_type)),
    }
}

pub struct Decoder {
    partial_data: VecDeque<u8>,
}
impl Decoder {
    pub fn new() -> Self {
        Decoder {
            partial_data: VecDeque::new(),
        }
    }

    pub fn decode(&mut self, data: Vec<u8>) -> Result<Vec<frame::Frame>, FrameDecodeError> {
        self.partial_data.append(&mut VecDeque::from(data));

        let mut decoded_frames = vec![];
        while self.partial_data.len() >= 3 {
            let body_len = double_u8_to_u16(
                self.partial_data[1], 
                self.partial_data[2],
            );

            // If we have at least a full frame, parse it
            if (self.partial_data.len() as u16) >= 3 + body_len {
                let index_after_last_frame_byte: usize = (3 + body_len).into();
                let mut frame_data = 
                    self.partial_data
                        .drain(0..index_after_last_frame_byte)
                        .collect::<VecDeque<_>>();

                let frame_type = match frame_data.pop_front() {
                    Some(ft) => ft,
                    None => {
                        let logic_error_str = concat!(
                            "Although partial_data size was big enough, ",
                            "pop_front returned None??"
                        );
                        return Err(FrameDecodeError {
                            parse_error: 
                                FrameParseError::InternalByteOffsetLogicError(
                                    logic_error_str.to_string()
                                ),
                            num_frames_parsed_successfully: decoded_frames.len(),
                        });
                    }
                };
                frame_data.pop_front(); // FrameBodyByteLength[0]
                frame_data.pop_front(); // FrameBodyByteLength[1]
                match parse_frame_body(frame_type, frame_data) {
                    Ok(frame) => decoded_frames.push(frame),
                    Err(decode_error) => return Err(FrameDecodeError {
                        parse_error: decode_error,
                        num_frames_parsed_successfully: decoded_frames.len(),
                    })
                }
            } else {
                break;
            }
        }
        
        Ok(decoded_frames)
    }
}

