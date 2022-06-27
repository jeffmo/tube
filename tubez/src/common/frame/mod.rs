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
        let tube_id = 65000;
        let encoded_bytes = encode_client_has_finished_sending_frame(tube_id).unwrap();

        let mut decoder = Decoder::new();
        let frames = decoder.decode(encoded_bytes).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], Frame::ClientHasFinishedSending { tube_id });
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
    fn establishtube_frame_encodes_and_decodes() {
        let tube_id = 65000;
        let encoded_headers = HashMap::from([
          ("header1".to_string(), "value1".to_string()),
          ("header2".to_string(), "value2".to_string()),
        ]);
        let expected_headers = encoded_headers.clone();

        let encoded_bytes = 
          encode_establish_tube_frame(tube_id, encoded_headers).unwrap();

        let mut decoder = Decoder::new();
        let frames = decoder.decode(encoded_bytes).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], Frame::EstablishTube {
          tube_id,
          headers: expected_headers,
        });
    }

    #[test]
    fn payload_frame_with_ack_encodes_and_decodes() {
        let tube_id = 65000;
        let ack_id = 32000;
        let data = vec![0, 1, 42, 255];
        let expected_data = data.clone();

        let encoded_bytes = encode_payload_frame(tube_id, Some(ack_id), data).unwrap();

        let mut decoder = Decoder::new();
        let frames = decoder.decode(encoded_bytes).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], Frame::Payload {
          tube_id,
          ack_id: Some(ack_id),
          data: expected_data,
        });
    }

    #[test]
    fn payload_frame_without_ack_encodes_and_decodes() {
        let tube_id = 65000;
        let data = vec![0, 1, 42, 255];
        let expected_data = data.clone();

        let encoded_bytes = encode_payload_frame(tube_id, None, data).unwrap();

        let mut decoder = Decoder::new();
        let frames = decoder.decode(encoded_bytes).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], Frame::Payload {
          tube_id,
          ack_id: None,
          data: expected_data,
        });
    }

    #[test]
    fn payload_ack_frame_encodes_and_decodes() {
        let tube_id = 65000;
        let ack_id: u16 = 32000;

        let encoded_bytes = encode_payload_ack_frame(tube_id, ack_id).unwrap();

        let mut decoder = Decoder::new();
        let frames = decoder.decode(encoded_bytes).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], Frame::PayloadAck {
          tube_id,
          ack_id,
        });
    }

    #[test]
    fn serverhasfinishedsending_frame_encodes_and_decodes() {
        let tube_id = 65000;
        let encoded_bytes = encode_server_has_finished_sending_frame(tube_id).unwrap();

        let mut decoder = Decoder::new();
        let frames = decoder.decode(encoded_bytes).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], Frame::ServerHasFinishedSending { tube_id });
    }
}

