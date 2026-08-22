use std::io::{self, BufWriter, Write};

use crate::error::{NtError, Result};

pub(crate) fn write_stream(
    output: &mut dyn Write,
    produce: impl FnOnce(&mut dyn Write) -> Result<()>,
) -> Result<()> {
    let mut output = BufWriter::new(output);
    match produce(&mut output) {
        Err(NtError::Io(error)) if error.kind() == io::ErrorKind::BrokenPipe => return Ok(()),
        result => result?,
    }
    match output.flush() {
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(error.into()),
        Ok(()) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};

    use super::write_stream;
    use crate::error::NtError;

    #[test]
    fn broken_pipe_during_write_or_flush_is_success() {
        write_stream(&mut BrokenPipeWriter, |output| {
            output.write_all(&[0; 16 * 1024])?;
            Ok(())
        })
        .unwrap();
        write_stream(&mut FlushBrokenPipeWriter, |output| {
            output.write_all(b"buffered")?;
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn non_pipe_errors_are_propagated() {
        let error = write_stream(&mut OtherErrorWriter, |output| {
            output.write_all(&[0; 16 * 1024])?;
            Ok(())
        })
        .unwrap_err();

        assert!(matches!(
            error,
            NtError::Io(error) if error.kind() == io::ErrorKind::Other
        ));
    }

    struct BrokenPipeWriter;
    struct FlushBrokenPipeWriter;
    struct OtherErrorWriter;

    impl Write for BrokenPipeWriter {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Write for FlushBrokenPipeWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
        }
    }

    impl Write for OtherErrorWriter {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("failed"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
}
