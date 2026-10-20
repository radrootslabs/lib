use crate::wire::MeteredIo;
use futures::{
    SinkExt, StreamExt,
    lock::Mutex,
    stream::{SplitSink, SplitStream},
};
use radroots_transport::host::websocket::{CloseFrame, Error, Frame, IoFuture, Socket};
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, tungstenite::Message};

type NativeStream = WebSocketStream<MeteredIo<MaybeTlsStream<TcpStream>>>;

pub(crate) struct NativeSocket {
    sink: Mutex<SplitSink<NativeStream, Message>>,
    stream: Mutex<SplitStream<NativeStream>>,
    max_message_bytes: usize,
}

impl NativeSocket {
    pub(crate) fn new(
        sink: SplitSink<NativeStream, Message>,
        stream: SplitStream<NativeStream>,
        max_message_bytes: usize,
    ) -> Self {
        Self {
            sink: Mutex::new(sink),
            stream: Mutex::new(stream),
            max_message_bytes,
        }
    }
}

impl Socket for NativeSocket {
    fn send(&self, frame: Frame) -> IoFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            let message = to_wire(frame, self.max_message_bytes)?;
            self.sink
                .lock()
                .await
                .send(message)
                .await
                .map_err(|_| Error::Unavailable)
        })
    }

    fn receive(&self) -> IoFuture<'_, Result<Option<Frame>, Error>> {
        Box::pin(async move {
            self.stream
                .lock()
                .await
                .next()
                .await
                .transpose()
                .map_err(|_| Error::Unavailable)?
                .map(from_wire)
                .transpose()
        })
    }

    fn close(&self) -> IoFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            self.sink
                .lock()
                .await
                .close()
                .await
                .map_err(|_| Error::Unavailable)
        })
    }
}

fn to_wire(frame: Frame, limit: usize) -> Result<Message, Error> {
    Ok(match frame {
        Frame::Text(value) if value.len() <= limit => Message::Text(value.into()),
        Frame::Binary(value) if value.len() <= limit => Message::Binary(value.into()),
        Frame::Ping(value) if value.len() <= 125 => Message::Ping(value.into()),
        Frame::Pong(value) if value.len() <= 125 => Message::Pong(value.into()),
        Frame::Close(None) => Message::Close(None),
        Frame::Close(Some(value)) if value.reason.len() <= 123 => {
            Message::Close(Some(tokio_tungstenite::tungstenite::protocol::CloseFrame {
                code: value.code.into(),
                reason: value.reason.into(),
            }))
        }
        _ => return Err(Error::Protocol),
    })
}

fn from_wire(message: Message) -> Result<Frame, Error> {
    Ok(match message {
        Message::Text(value) => Frame::Text(value.to_string()),
        Message::Binary(value) => Frame::Binary(value.to_vec()),
        Message::Ping(value) => Frame::Ping(value.to_vec()),
        Message::Pong(value) => Frame::Pong(value.to_vec()),
        Message::Close(value) => Frame::Close(value.map(|value| CloseFrame {
            code: value.code.into(),
            reason: value.reason.to_string(),
        })),
        Message::Frame(_) => return Err(Error::Protocol),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_control_and_original_close_information_round_trip_without_reencoding() {
        for frame in [
            Frame::Text(" \n{ \"z\":1, \"a\":2 } \t".into()),
            Frame::Binary(vec![0, 255]),
            Frame::Ping(vec![1]),
            Frame::Pong(vec![2]),
            Frame::Close(None),
            Frame::Close(Some(CloseFrame {
                code: 1000,
                reason: "complete".into(),
            })),
        ] {
            assert_eq!(
                from_wire(to_wire(frame.clone(), 1024).unwrap()).unwrap(),
                frame
            );
        }
    }

    #[test]
    fn every_outgoing_data_and_control_bound_is_checked_before_socket_io() {
        for frame in [Frame::Text("x".repeat(64)), Frame::Binary(vec![1; 64])] {
            assert!(to_wire(frame, 64).is_ok());
        }
        for frame in [
            Frame::Text("x".repeat(65)),
            Frame::Binary(vec![1; 65]),
            Frame::Ping(vec![1; 126]),
            Frame::Pong(vec![1; 126]),
            Frame::Close(Some(CloseFrame {
                code: 1000,
                reason: "x".repeat(124),
            })),
        ] {
            assert_eq!(to_wire(frame, 64).err(), Some(Error::Protocol));
        }
        for frame in [
            Frame::Ping(vec![1; 125]),
            Frame::Pong(vec![1; 125]),
            Frame::Close(Some(CloseFrame {
                code: 1000,
                reason: "x".repeat(123),
            })),
        ] {
            assert!(to_wire(frame, 64).is_ok());
        }
        let raw = tokio_tungstenite::tungstenite::protocol::frame::Frame::message(
            vec![1],
            tokio_tungstenite::tungstenite::protocol::frame::coding::OpCode::Data(
                tokio_tungstenite::tungstenite::protocol::frame::coding::Data::Binary,
            ),
            true,
        );
        assert_eq!(from_wire(Message::Frame(raw)).err(), Some(Error::Protocol));
    }
}
