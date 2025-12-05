//! Ring buffer implementation for bounded output capture
//!
//! Provides fixed-size ring buffers for stdout and stderr capture with automatic truncation.

use std::collections::VecDeque;
use std::fmt;

/// A ring buffer that maintains a fixed maximum size
#[derive(Debug, Clone)]
pub struct RingBuffer {
    buffer: VecDeque<u8>,
    max_bytes: usize,
    total_bytes_written: usize,
}

impl RingBuffer {
    /// Create a new ring buffer with the specified maximum size
    #[must_use]
    pub fn new(max_bytes: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(max_bytes.min(8192)),
            max_bytes,
            total_bytes_written: 0,
        }
    }

    /// Write data to the ring buffer
    ///
    /// If the buffer would exceed `max_bytes`, old data is dropped from the front.
    pub fn write(&mut self, data: &[u8]) {
        self.total_bytes_written += data.len();

        for &byte in data {
            if self.buffer.len() >= self.max_bytes {
                // Buffer is full, remove oldest byte
                self.buffer.pop_front();
            }
            self.buffer.push_back(byte);
        }
    }

    /// Get the current size of the buffer in bytes
    #[must_use]
    #[allow(dead_code)] // Standard collection API method
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Check if the buffer is empty
    #[must_use]
    #[allow(dead_code)] // Standard collection API method
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Get the total number of bytes written (including truncated bytes)
    #[must_use]
    pub const fn total_bytes_written(&self) -> usize {
        self.total_bytes_written
    }

    /// Check if any data was truncated
    #[must_use]
    pub const fn was_truncated(&self) -> bool {
        self.total_bytes_written > self.max_bytes
    }
}

impl fmt::Display for RingBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes: Vec<u8> = self.buffer.iter().copied().collect();
        write!(f, "{}", String::from_utf8_lossy(&bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_basic() {
        let mut buffer = RingBuffer::new(10);
        buffer.write(b"hello");
        assert_eq!(buffer.to_string(), "hello");
        assert_eq!(buffer.len(), 5);
        assert!(!buffer.is_empty());
    }

    #[test]
    fn test_ring_buffer_truncation() {
        let mut buffer = RingBuffer::new(10);
        buffer.write(b"hello");
        buffer.write(b"world");
        buffer.write(b"!");

        // Total written: 11 bytes, but buffer only holds 10
        assert_eq!(buffer.len(), 10);
        assert_eq!(buffer.to_string(), "elloworld!");
        assert_eq!(buffer.total_bytes_written(), 11);
        assert!(buffer.was_truncated());
    }

    #[test]
    fn test_ring_buffer_large_write() {
        let mut buffer = RingBuffer::new(5);
        buffer.write(b"hello world");

        // Should only keep the last 5 bytes
        assert_eq!(buffer.len(), 5);
        assert_eq!(buffer.to_string(), "world");
        assert_eq!(buffer.total_bytes_written(), 11);
        assert!(buffer.was_truncated());
    }

    #[test]
    fn test_ring_buffer_exact_capacity() {
        let mut buffer = RingBuffer::new(10);
        buffer.write(b"1234567890");

        assert_eq!(buffer.len(), 10);
        assert_eq!(buffer.to_string(), "1234567890");
        assert!(!buffer.was_truncated());
    }

    #[test]
    fn test_ring_buffer_multiple_writes() {
        let mut buffer = RingBuffer::new(10);
        buffer.write(b"12345");
        buffer.write(b"67890");
        buffer.write(b"ABCDE");

        // Should keep last 10 bytes: "67890ABCDE"
        assert_eq!(buffer.len(), 10);
        assert_eq!(buffer.to_string(), "67890ABCDE");
        assert_eq!(buffer.total_bytes_written(), 15);
        assert!(buffer.was_truncated());
    }

    #[test]
    fn test_ring_buffer_empty() {
        let buffer = RingBuffer::new(10);
        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.to_string(), "");
        assert!(!buffer.was_truncated());
    }

    #[test]
    fn test_ring_buffer_utf8_handling() {
        let mut buffer = RingBuffer::new(20);
        buffer.write("Hello 世界".as_bytes());
        assert_eq!(buffer.to_string(), "Hello 世界");
    }

    #[test]
    fn test_ring_buffer_invalid_utf8() {
        let mut buffer = RingBuffer::new(10);
        // Write invalid UTF-8 sequence
        buffer.write(&[0xFF, 0xFE, 0xFD]);
        // Should use replacement character
        let result = buffer.to_string();
        assert!(!result.is_empty());
    }
}
