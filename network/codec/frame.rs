use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io;
use std::marker::Unpin;

/// Maximum message size in bytes
const MAX_MESSAGE_SIZE: u32 = 10 * 1024 * 1024; // 10 MB

/// Reader for framed messages
pub struct FramedReader<R> {
    reader: R,
}

impl<R: AsyncRead + Unpin> FramedReader<R> {
    /// Create a new framed reader
    pub fn new(reader: R) -> Self {
        Self { reader }
    }
    
    /// Read a message from the stream
    pub async fn read_message<T: DeserializeOwned>(&mut self) -> bincode::Result<Option<T>> {
        // Read the message length
        let len = match self.reader.read_u32().await {
            Ok(len) => len,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(bincode::Error::Io(e)),
        };
        
        // Check message size
        if len > MAX_MESSAGE_SIZE {
            return Err(bincode::Error::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Message too large: {} bytes", len),
            )));
        }
        
        // Read the message data
        let mut buf = vec![0u8; len as usize];
        match self.reader.read_exact(&mut buf).await {
            Ok(_) => (),
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(bincode::Error::Io(e)),
        }
        
        // Deserialize the message
        Ok(Some(bincode::deserialize(&buf)?))
    }
}

/// Writer for framed messages
pub struct FramedWriter<W> {
    writer: W,
}

impl<W: AsyncWrite + Unpin> FramedWriter<W> {
    /// Create a new framed writer
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
    
    /// Write a message to the stream
    pub async fn write_message<T: Serialize>(&mut self, message: &T) -> bincode::Result<()> {
        // Serialize the message
        let data = bincode::serialize(message)?;
        
        // Check message size
        if data.len() > MAX_MESSAGE_SIZE as usize {
            return Err(bincode::Error::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Message too large: {} bytes", data.len()),
            )));
        }
        
        // Write the message length
        self.writer.write_u32(data.len() as u32).await
            .map_err(bincode::Error::Io)?;
        
        // Write the message data
        self.writer.write_all(&data).await
            .map_err(bincode::Error::Io)?;
        
        // Flush the writer
        self.writer.flush().await
            .map_err(bincode::Error::Io)?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Serialize, Deserialize};
    use tokio::io::duplex;
    
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestMessage {
        id: u32,
        data: String,
    }
    
    #[tokio::test]
    async fn test_framed_codec() {
        // Create a duplex channel
        let (client, server) = duplex(1024);
        
        // Create reader and writer
        let mut reader = FramedReader::new(client);
        let mut writer = FramedWriter::new(server);
        
        // Create a test message
        let message = TestMessage {
            id: 42,
            data: "Hello, world!".to_string(),
        };
        
        // Write the message
        writer.write_message(&message).await.unwrap();
        
        // Read the message
        let received: TestMessage = reader.read_message().await.unwrap().unwrap();
        
        // Check that we got the same message
        assert_eq!(received, message);
    }
    
    #[tokio::test]
    async fn test_multiple_messages() {
        // Create a duplex channel
        let (client, server) = duplex(1024);
        
        // Create reader and writer
        let mut reader = FramedReader::new(client);
        let mut writer = FramedWriter::new(server);
        
        // Create test messages
        let messages = vec![
            TestMessage { id: 1, data: "First".to_string() },
            TestMessage { id: 2, data: "Second".to_string() },
            TestMessage { id: 3, data: "Third".to_string() },
        ];
        
        // Write the messages
        for message in &messages {
            writer.write_message(message).await.unwrap();
        }
        
        // Read the messages
        for expected in &messages {
            let received: TestMessage = reader.read_message().await.unwrap().unwrap();
            assert_eq!(&received, expected);
        }
    }
}
