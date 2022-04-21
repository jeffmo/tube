use std::collections::VecDeque;

use super::Frame;

/**
 * Each Streamr frame specifies its own structure, but all frames begin with the 
 * following header structure:
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

pub enum FrameParseError {
    UnexpectedFrameType,
}

fn double_u8_to_u16(left_byte: u8, right_byte: u8) -> u16 {
    // 1) LLLLLLLL -> 00000000LLLLLLLL
    // 2) 00000000LLLLLLLL -> LLLLLLLL00000000
    // 3) LLLLLLLL00000000 -> LLLLLLLLRRRRRRRR
    ((left_byte as u16) << 8) | (right_byte as u16)
}

fn parse_frame(body_len: u16, mut single_frame_data: VecDeque<u8>) 
        -> Result<Frame, FrameParseError> {
    let frame_type = single_frame_data.pop_front();
    single_frame_data.pop_front(); // FrameBodyByteLength[0]
    single_frame_data.pop_front(); // FrameBodyByteLength[1]

    match frame_type {
        // ClientHasFinishedSending
        Some(0) => {
            let streamr_id = double_u8_to_u16(
                single_frame_data[0],
                single_frame_data[1],
            );
            Ok(Frame::ClientHasFinishedSending { streamr_id })
        },

        // Drain
        Some(1) => {
            Ok(Frame::Drain)
        },

        // EstablishStreamr
        Some(2) => {
            let streamr_id = TODO;
            let headers = TODO;
            Ok(Frame::EstablishStreamr { streamr_id, headers })
        },

        /*
        // Payload
        Some(3) => {
            let streamr_id = TODO;
            let ack_id = TODO;
            let data = TODO;
            Ok(Frame::Payload { streamr_id, ack_id, data })
        },

        // PayloadAck
        Some(4) => {
            let streamr_id = TODO;
            let ack_id = TODO;
            Ok(Frame::PayloadAck { streamr_id, ack_id })
        },
        */

        // ServerHasFinishedSending
        Some(5) => {
            let streamr_id = double_u8_to_u16(
                single_frame_data[0],
                single_frame_data[1],
            );
            Ok(Frame::ServerHasFinishedSending { streamr_id })
        },

        _ => Err(FrameParseError::UnexpectedFrameType)
    }
}

pub struct FrameDecoder {
    partial_data: VecDeque<u8>,
}
impl FrameDecoder {
    pub fn new() -> Self {
        FrameDecoder {
            partial_data: VecDeque::new(),
        }
    }

    pub fn decode(mut self, data: Vec<u8>) -> Vec<Frame> {
        self.partial_data.append(&mut VecDeque::from(data));

        let mut decoded_frames = vec![];
        while self.partial_data.len() >= 3 {
            let body_len = double_u8_to_u16(
                self.partial_data[1], 
                self.partial_data[2],
            );

            // If we have at least a full frame, parse it
            if (self.partial_data.len() as u16) >= 3 + body_len {
                let index_after_last_frame_byte = 3 + body_len;
                let remainder_data = 
                    self.partial_data.split_off(index_after_last_frame_byte.into());

                let frame_data = self.partial_data;
                self.partial_data = remainder_data;

                match parse_frame(body_len, frame_data) {
                    Ok(frame) => decoded_frames.push(frame),

                    // TODO: Do something with this error?
                    Err(decode_error) => ()
                }
            }
        }
        
        decoded_frames
    }
}
