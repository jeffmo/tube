pub use decoder::Decoder;
pub use decoder::FrameDecodeError;
pub use decoder::FrameParseError;
pub use encoder::*;
pub use frame::Frame;

mod decoder;
mod encoder;
mod frame;

#[cfg(test)]
mod codec_tests {
    use std::collections::HashMap;

    use super::Frame;
    use super::Decoder;
    use super::encoder::*;

    #[test]
    fn clienthasfinishedsending_frame_encodes_and_decodes() {
        let streamr_id = 65000;
        let encoded_bytes = encode_client_has_finished_sending_frame(streamr_id).unwrap();

        let mut decoder = Decoder::new();
        let frames = decoder.decode(encoded_bytes).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], Frame::ClientHasFinishedSending { streamr_id });
    }

    #[test]
    fn drain_frame_encodes_and_decodes() {
        let encoded_bytes = encode_drain_frame().unwrap();

        let mut decoder = Decoder::new();
        let frames = decoder.decode(encoded_bytes).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], Frame::Drain);
    }

    #[test]
    fn establishstreamr_frame_encodes_and_decodes() {
        let streamr_id = 65000;
        let encoded_headers = HashMap::from([
          ("header1".to_string(), "value1".to_string()),
          ("header2".to_string(), "value2".to_string()),
        ]);
        let expected_headers = encoded_headers.clone();

        let encoded_bytes = 
          encode_establish_streamr_frame(streamr_id, encoded_headers).unwrap();

        let mut decoder = Decoder::new();
        let frames = decoder.decode(encoded_bytes).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], Frame::EstablishStreamr {
          streamr_id,
          headers: expected_headers,
        });
    }

    #[test]
    fn payload_frame_with_ack_encodes_and_decodes() {
        let streamr_id = 65000;
        let ack_id = 32000;
        let data = vec![0, 1, 42, 255];
        let expected_data = data.clone();

        let encoded_bytes = encode_payload_frame(streamr_id, Some(ack_id), data).unwrap();

        let mut decoder = Decoder::new();
        let frames = decoder.decode(encoded_bytes).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], Frame::Payload {
          streamr_id,
          ack_id: Some(ack_id),
          data: expected_data,
        });
    }

    #[test]
    fn payload_frame_without_ack_encodes_and_decodes() {
        let streamr_id = 65000;
        let data = vec![0, 1, 42, 255];
        let expected_data = data.clone();

        let encoded_bytes = encode_payload_frame(streamr_id, None, data).unwrap();

        let mut decoder = Decoder::new();
        let frames = decoder.decode(encoded_bytes).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], Frame::Payload {
          streamr_id,
          ack_id: None,
          data: expected_data,
        });
    }

    #[test]
    fn payload_ack_frame_encodes_and_decodes() {
        let streamr_id = 65000;
        let ack_id: u16 = 32000;

        let encoded_bytes = encode_payload_ack_frame(streamr_id, ack_id).unwrap();

        let mut decoder = Decoder::new();
        let frames = decoder.decode(encoded_bytes).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], Frame::PayloadAck {
          streamr_id,
          ack_id,
        });
    }

    #[test]
    fn serverhasfinishedsending_frame_encodes_and_decodes() {
        let streamr_id = 65000;
        let encoded_bytes = encode_server_has_finished_sending_frame(streamr_id).unwrap();

        let mut decoder = Decoder::new();
        let frames = decoder.decode(encoded_bytes).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], Frame::ServerHasFinishedSending { streamr_id });
    }
}

#[cfg(test)]
mod decoder_tests {
    use std::collections::HashMap;

    use super::Decoder;
    use super::encoder::*;
    use super::Frame;
    use super::FrameDecodeError;
    use super::FrameParseError;

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
        let mut data = encode_client_has_finished_sending_frame(43).unwrap();

        let final_byte = data.pop().unwrap();
        let decoded_frames = &decoder.decode(data).unwrap();
        assert_eq!(decoded_frames.len(), 0);

        let decoded_frames = &decoder.decode(vec![final_byte]).unwrap();
        assert_eq!(decoded_frames.len(), 1);
        assert_eq!(decoded_frames[0], Frame::ClientHasFinishedSending { streamr_id: 43 });
    }

    #[test]
    fn data_for_two_full_frames_yield_two_frames() {
        let mut decoder = Decoder::new();

        let mut data = encode_client_has_finished_sending_frame(43).unwrap();
        data.append(&mut encode_server_has_finished_sending_frame(42).unwrap());

        let decoded_frames = &decoder.decode(data).unwrap();
        assert_eq!(decoded_frames.len(), 2);
        assert_eq!(decoded_frames[0], Frame::ClientHasFinishedSending { streamr_id: 43 });
        assert_eq!(decoded_frames[1], Frame::ServerHasFinishedSending { streamr_id: 42 });
    }

    #[test]
    fn full_frame_plus_partial_frame_yields_single_frame_until_rest_of_second_frame_provided() {
        let mut decoder = Decoder::new();

        let mut data = encode_client_has_finished_sending_frame(43).unwrap();
        data.append(&mut encode_server_has_finished_sending_frame(42).unwrap());
        let final_byte = data.pop().unwrap();

        let decoded_frames = &decoder.decode(data).unwrap();
        assert_eq!(decoded_frames.len(), 1);
        assert_eq!(decoded_frames[0], Frame::ClientHasFinishedSending { streamr_id: 43 });

        let decoded_frames = &decoder.decode(vec![final_byte]).unwrap();
        assert_eq!(decoded_frames.len(), 1);
        assert_eq!(decoded_frames[0], Frame::ServerHasFinishedSending { streamr_id: 42 });
    }

    #[test]
    fn errors_if_invalid_utf8_passed_for_establishstreamr_headers() {
        let mut decoder = Decoder::new();

        let headers = HashMap::from([
          ("header1".to_string(), "value1".to_string()),
          ("header2".to_string(), "value2".to_string()),
        ]);
        let mut data = encode_establish_streamr_frame(42, headers).unwrap();

        // Tweak encoded data to insert an invalid utf8 byte into the encoded 
        // headers region of the frame.
        data.pop();
        data.push(159);

        match &decoder.decode(data) {
            Ok(_frames) => panic!(concat!(
                "Successfully decoded an EstablishStreamr frame that contains ",
                "invalid utf8!"
            )),

            Err(FrameDecodeError{
              parse_error: FrameParseError::HeaderUtf8Error(utf8_err),
              ..
            }) => assert_eq!(utf8_err.valid_up_to(), 38),

            Err(err) => panic!(
                concat!(
                    "Correctly errored on an EstablishStreamr frame with ",
                    "invalid utf8, but provided an unexpected error value: {:?}"
                ), 
                err
            )
        };
    }

    #[test]
    fn errors_if_invalid_json_passed_for_establishstreamr_headers() {
        let mut decoder = Decoder::new();

        let headers = HashMap::from([]);
        let correct_data = encode_establish_streamr_frame(42, headers).unwrap();

        // Tweak encoded data to insert invalid json into the headers portion 
        // of the frame.
        let mut bad_data = vec![
          // FrameType
          correct_data[0],

          // FrameBodyByteLength
          correct_data[1],
          correct_data[2],

          // StreamrId
          correct_data[3],
          correct_data[4],
        ];

        let mut invalid_json_bytes = "{]".to_string().into_bytes();
        bad_data.append(&mut invalid_json_bytes);

        match &decoder.decode(bad_data) {
            Ok(_frames) => panic!(concat!(
                "Successfully decoded an EstablishStreamr frame that contains ",
                "an invalid json encoding of it's headers!"
            )),

            Err(FrameDecodeError {
              parse_error: FrameParseError::HeaderJsonDecodeError(_),
              ..
            }) => assert!(true),

            Err(err) => panic!(
                concat!(
                    "Correctly errored on an EstablishStreamr frame with invalid ",
                    "utf8, but provided an unexpected error value: {:?}"
                ), 
                err
            )
        };
    }

    #[test]
    fn errors_if_frametype_value_is_unknown() {
        let mut decoder = Decoder::new();

        let mut data = encode_client_has_finished_sending_frame(43).unwrap();

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
