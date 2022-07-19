use std::collections::HashMap;
use std::collections::VecDeque;

use serde_json;

use super::frame;

// Returned by Decoder::decode() and provides context around 
// a FrameParseError
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
        frame::CLIENT_HAS_FINISHED_SENDING_FRAMETYPE => {
            let tube_id = double_u8_to_u16(
                frame_body_data[0],
                frame_body_data[1],
            );
            Ok(frame::Frame::ClientHasFinishedSending { tube_id })
        },

       frame::DRAIN_FRAMETYPE => {
            Ok(frame::Frame::Drain)
        },

        frame::NEWTUBE_FRAMETYPE => {
            let mut header_bytes = frame_body_data.split_off(2);
            let tube_id = double_u8_to_u16(
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
            Ok(frame::Frame::NewTube { tube_id, headers })
        },

        frame::PAYLOAD_FRAMETYPE => {
            let data = frame_body_data.split_off(4).make_contiguous().to_vec();
            let tube_id = double_u8_to_u16(
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
            Ok(frame::Frame::Payload { tube_id, ack_id, data })
        },

        frame::PAYLOAD_ACK_FRAMETYPE => {
            let tube_id = double_u8_to_u16(
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
            Ok(frame::Frame::PayloadAck { tube_id, ack_id })
        },

        frame::SERVER_HAS_FINISHED_SENDING_FRAMETYPE => {
            let tube_id = double_u8_to_u16(
                frame_body_data[0],
                frame_body_data[1],
            );
            Ok(frame::Frame::ServerHasFinishedSending { tube_id })
        },

        frame::ABORT_FRAMETYPE => {
            let tube_id = double_u8_to_u16(
                frame_body_data[0],
                frame_body_data[1],
            );
            let reason = frame::AbortReason::from(frame_body_data[2]);
            Ok(frame::Frame::Abort {
                tube_id,
                reason,
            })
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

    pub fn decode(
        &mut self, 
        data: Vec<u8>,
    ) -> Result<VecDeque<frame::Frame>, FrameDecodeError> {
        self.partial_data.append(&mut VecDeque::from(data));

        let mut decoded_frames = VecDeque::new();
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
                    Ok(frame) => decoded_frames.push_back(frame),
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

#[cfg(test)]
mod decoder_tests {
    use std::collections::HashMap;

    use super::*;
    use super::super::encoder;

    #[test]
    fn empty_data_yields_empty_vec() {
        let mut decoder = Decoder::new();
        let data = vec![];
        let decoded_frames = decoder.decode(data).unwrap();
        assert_eq!(decoded_frames.len(), 0);
    }

    #[test]
    fn partial_data_yields_empty_vec_until_rest_of_data_provided() {
        let mut decoder = Decoder::new();
        let mut data = encoder::encode_client_has_finished_sending_frame(43).unwrap();

        let final_byte = data.pop().unwrap();
        let decoded_frames = &decoder.decode(data).unwrap();
        assert_eq!(decoded_frames.len(), 0);

        let decoded_frames = &decoder.decode(vec![final_byte]).unwrap();
        assert_eq!(decoded_frames.len(), 1);
        assert_eq!(decoded_frames[0], frame::Frame::ClientHasFinishedSending { tube_id: 43 });
    }

    #[test]
    fn data_for_two_full_frames_yield_two_frames() {
        let mut decoder = Decoder::new();

        let mut data = encoder::encode_client_has_finished_sending_frame(43).unwrap();
        data.append(&mut encoder::encode_server_has_finished_sending_frame(42).unwrap());

        let decoded_frames = &decoder.decode(data).unwrap();
        assert_eq!(decoded_frames.len(), 2);
        assert_eq!(decoded_frames[0], frame::Frame::ClientHasFinishedSending { tube_id: 43 });
        assert_eq!(decoded_frames[1], frame::Frame::ServerHasFinishedSending { tube_id: 42 });
    }

    #[test]
    fn full_frame_plus_partial_frame_yields_single_frame_until_rest_of_second_frame_provided() {
        let mut decoder = Decoder::new();

        let mut data = encoder::encode_client_has_finished_sending_frame(43).unwrap();
        data.append(&mut encoder::encode_server_has_finished_sending_frame(42).unwrap());
        let final_byte = data.pop().unwrap();

        let decoded_frames = &decoder.decode(data).unwrap();
        assert_eq!(decoded_frames.len(), 1);
        assert_eq!(decoded_frames[0], frame::Frame::ClientHasFinishedSending { tube_id: 43 });

        let decoded_frames = &decoder.decode(vec![final_byte]).unwrap();
        assert_eq!(decoded_frames.len(), 1);
        assert_eq!(decoded_frames[0], frame::Frame::ServerHasFinishedSending { tube_id: 42 });
    }

    #[test]
    fn errors_if_invalid_utf8_passed_for_newtube_headers() {
        let mut decoder = Decoder::new();

        let headers = HashMap::from([
          ("header1".to_string(), "value1".to_string()),
          ("header2".to_string(), "value2".to_string()),
        ]);
        let mut data = encoder::encode_newtube_frame(42, headers).unwrap();

        // Tweak encoded data to insert an invalid utf8 byte into the encoded 
        // headers region of the frame.
        data.pop();
        data.push(159);

        match &decoder.decode(data) {
            Ok(_frames) => panic!(concat!(
                "Successfully decoded a NewTube frame that contains ",
                "invalid utf8!"
            )),

            Err(FrameDecodeError{
              parse_error: FrameParseError::HeaderUtf8Error(utf8_err),
              ..
            }) => assert_eq!(utf8_err.valid_up_to(), 38),

            Err(err) => panic!(
                concat!(
                    "Correctly errored on a NewTube frame with ",
                    "invalid utf8, but provided an unexpected error value: {:?}"
                ), 
                err
            )
        };
    }

    #[test]
    fn errors_if_invalid_json_passed_for_newtube_headers() {
        let mut decoder = Decoder::new();

        let headers = HashMap::from([]);
        let correct_data = encoder::encode_newtube_frame(42, headers).unwrap();

        // Tweak encoded data to insert invalid json into the headers portion 
        // of the frame.
        let mut bad_data = vec![
          // FrameType
          correct_data[0],

          // FrameBodyByteLength
          correct_data[1],
          correct_data[2],

          // TubeId
          correct_data[3],
          correct_data[4],
        ];

        let mut invalid_json_bytes = "{]".to_string().into_bytes();
        bad_data.append(&mut invalid_json_bytes);

        match &decoder.decode(bad_data) {
            Ok(_frames) => panic!(concat!(
                "Successfully decoded a NewTube frame that contains ",
                "an invalid json encoding of it's headers!"
            )),

            Err(FrameDecodeError {
              parse_error: FrameParseError::HeaderJsonDecodeError(_),
              ..
            }) => assert!(true),

            Err(err) => panic!(
                concat!(
                    "Correctly errored on a NewTube frame with invalid ",
                    "utf8, but provided an unexpected error value: {:?}"
                ), 
                err
            )
        };
    }

    #[test]
    fn errors_if_frametype_value_is_unknown() {
        let mut decoder = Decoder::new();

        let mut data = encoder::encode_client_has_finished_sending_frame(43).unwrap();

        // Tweak encoded data to use an invalid FrameType value
        data[0] = 255;

        match &decoder.decode(data) {
            Ok(_frames) => panic!("Somehow decoded a frame with an invalid FrameType!?"),

            Err(FrameDecodeError{
              parse_error: FrameParseError::UnknownFrameType(frametype_value),
              ..
            }) => assert_eq!(*frametype_value, 255),

            Err(err) => panic!(
                concat!(
                    "Correctly errored on a frame with an invalid FrameType, ",
                    "but provided an unexpected error valie: {:?}"
                ), 
                err
            )
        };
    }
}

