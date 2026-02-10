//! RESP (Redis Serialization Protocol) parser and formatter.
//!
//! This module provides utilities to convert Redis values back to RESP frames
//! and to parse incoming raw RESP commands from a byte buffer.

use redis::Value;
use std::io::{BufRead, Cursor, Read};

/// Errors that can occur during RESP parsing.
#[derive(Debug)]
pub enum RespError {
    /// The input data does not follow the RESP format.
    InvalidFormat,
    /// The input data is incomplete (e.g., missing CRLF or expected bulk data).
    Incomplete,
    /// An underlying I/O error occurred.
    Io(std::io::Error),
}

/// Converts a `redis::Value` into its RESP byte representation.
///
/// This is used to send Redis responses back to the client over a raw WebSocket.
pub fn value_to_resp(v: &Value) -> Vec<u8> {
    match v {
        Value::Nil => b"$-1\r\n".to_vec(),
        Value::Int(i) => format!(":{}\r\n", i).into_bytes(),
        Value::BulkString(bytes) => {
            let mut res = format!("${}\r\n", bytes.len()).into_bytes();
            res.extend_from_slice(bytes);
            res.extend_from_slice(b"\r\n");
            res
        }
        Value::Array(items) => {
            let mut res = format!("*{}\r\n", items.len()).into_bytes();
            for item in items {
                res.extend_from_slice(&value_to_resp(item));
            }
            res
        }
        Value::SimpleString(s) => format!("+{}\r\n", s).into_bytes(),
        Value::Okay => b"+OK\r\n".to_vec(),
        _ => b"-ERR Unsupported RESP3 type\r\n".to_vec(),
    }
}

/// Parses a RESP command (Array of Bulk Strings) from a byte buffer.
///
/// Returns `Ok(Some((args, consumed)))` if a full command was parsed,
/// where `args` is a list of arguments (command name and params) and `consumed`
/// is the number of bytes read from the buffer.
///
/// Returns `Ok(None)` if more data is needed to complete the command.
pub fn parse_command(buffer: &[u8]) -> Result<Option<(Vec<Vec<u8>>, usize)>, RespError> {
    let mut cursor = Cursor::new(buffer);
    let mut line = String::new();

    // Read the array header (e.g., "*3\r\n")
    if cursor.read_line(&mut line).map_err(RespError::Io)? == 0 {
        return Ok(None);
    }

    if !line.starts_with('*') {
        // We only support the RESP Array format for commands on the raw endpoint.
        return Err(RespError::InvalidFormat);
    }

    // Parse number of arguments in the array
    let count: usize = line[1..]
        .trim()
        .parse()
        .map_err(|_| RespError::InvalidFormat)?;
    let mut args = Vec::with_capacity(count);

    // Parse each Bulk String argument
    for _ in 0..count {
        line.clear();
        // Read bulk string length header (e.g., "$5\r\n")
        if cursor.read_line(&mut line).map_err(RespError::Io)? == 0 {
            return Ok(None);
        }
        if !line.starts_with('$') {
            return Err(RespError::InvalidFormat);
        }
        let len: usize = line[1..]
            .trim()
            .parse()
            .map_err(|_| RespError::InvalidFormat)?;

        // Read the actual data
        let mut arg = vec![0u8; len];
        if cursor.read_exact(&mut arg).is_err() {
            return Ok(None); // Incomplete bulk string
        }

        // Consume the trailing CRLF
        let mut crlf = [0u8; 2];
        if cursor.read_exact(&mut crlf).is_err() {
            return Ok(None);
        }
        if &crlf != b"\r\n" {
            return Err(RespError::InvalidFormat);
        }

        args.push(arg);
    }

    // Return the parsed arguments and the total bytes consumed
    Ok(Some((args, cursor.position() as usize)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_to_resp() {
        assert_eq!(value_to_resp(&Value::Okay), b"+OK\r\n");
        assert_eq!(value_to_resp(&Value::Int(42)), b":42\r\n");
        assert_eq!(
            value_to_resp(&Value::BulkString(b"hello".to_vec())),
            b"$5\r\nhello\r\n"
        );
        assert_eq!(
            value_to_resp(&Value::Array(vec![Value::Int(1), Value::Okay])),
            b"*2\r\n:1\r\n+OK\r\n"
        );
    }

    #[test]
    fn test_parse_command() {
        let input = b"*2\r\n$3\r\nGET\r\n$4\r\nNAME\r\n";
        let (args, consumed) = parse_command(input).unwrap().unwrap();
        assert_eq!(args, vec![b"GET".to_vec(), b"NAME".to_vec()]);
        assert_eq!(consumed, input.len());
    }

    #[test]
    fn test_parse_incomplete() {
        let input = b"*2\r\n$3\r\nGET\r\n";
        assert!(parse_command(input).unwrap().is_none());
    }
}
