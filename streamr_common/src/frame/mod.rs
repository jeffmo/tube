pub use frame::Frame;
pub use decoder::Decoder;
pub use encoder::*;

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
        let encoded_bytes = match encode_client_has_finished_sending_frame(streamr_id) {
            Ok(bytes) => bytes,
            Err(err) => panic!("Encoding a ClientHasFinishedSending frame errored: {:?}", err)
        };

        let decoder = Decoder::new();
        let frames = match decoder.decode(encoded_bytes) {
            Ok(frame) => frame,
            Err(err) => panic!("Decoding ClientHasFinishedSending bytes errored: {:?}", err)
        };

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], Frame::ClientHasFinishedSending { streamr_id });
    }

    #[test]
    fn drain_frame_encodes_and_decodes() {
        let encoded_bytes = match encode_drain_frame() {
            Ok(bytes) => bytes,
            Err(err) => panic!("Encoding a Drain frame errored: {:?}", err)
        };

        let decoder = Decoder::new();
        let frames = match decoder.decode(encoded_bytes) {
            Ok(frame) => frame,
            Err(err) => panic!("Decoding Drain bytes errored: {:?}", err)
        };

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

        let encoded_bytes = match encode_establish_streamr_frame(streamr_id, encoded_headers) {
            Ok(bytes) => bytes,
            Err(err) => panic!("Encoding a EstablishStreamr frame errored: {:?}", err)
        };

        let decoder = Decoder::new();
        let frames = match decoder.decode(encoded_bytes) {
            Ok(frame) => frame,
            Err(err) => panic!("Decoding EstablishStreamr bytes errored: {:?}", err)
        };

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

        let encoded_bytes = match encode_payload_frame(streamr_id, Some(ack_id), data) {
            Ok(bytes) => bytes,
            Err(err) => panic!("Encoding a Payload frame (with ack) errored: {:?}", err)
        };

        let decoder = Decoder::new();
        let frames = match decoder.decode(encoded_bytes) {
            Ok(frame) => frame,
            Err(err) => panic!("Decoding Payload frame (with ack) bytes errored: {:?}", err)
        };

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

        let encoded_bytes = match encode_payload_frame(streamr_id, None, data) {
            Ok(bytes) => bytes,
            Err(err) => panic!("Encoding a Payload frame (without ack) errored: {:?}", err)
        };

        let decoder = Decoder::new();
        let frames = match decoder.decode(encoded_bytes) {
            Ok(frame) => frame,
            Err(err) => panic!("Decoding Payload frame (without ack) bytes errored: {:?}", err)
        };

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

        let encoded_bytes = match encode_payload_ack_frame(streamr_id, ack_id) {
            Ok(bytes) => bytes,
            Err(err) => panic!("Encoding a PayloadAck frame errored: {:?}", err)
        };

        let decoder = Decoder::new();
        let frames = match decoder.decode(encoded_bytes) {
            Ok(frame) => frame,
            Err(err) => panic!("Decoding PayloadAck frame bytes errored: {:?}", err)
        };

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], Frame::PayloadAck {
          streamr_id,
          ack_id,
        });
    }

    #[test]
    fn serverhasfinishedsending_frame_encodes_and_decodes() {
        let streamr_id = 65000;
        let encoded_bytes = match encode_server_has_finished_sending_frame(streamr_id) {
            Ok(bytes) => bytes,
            Err(err) => panic!("Encoding a ServerHasFinishedSending frame errored: {:?}", err)
        };

        let decoder = Decoder::new();
        let frames = match decoder.decode(encoded_bytes) {
            Ok(frame) => frame,
            Err(err) => panic!("Decoding ServerHasFinishedSending bytes errored: {:?}", err)
        };

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], Frame::ServerHasFinishedSending { streamr_id });
    }
}
