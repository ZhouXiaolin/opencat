use bytes::{Buf, BufMut};
use four_cc::FourCC;
pub use safer_bytes::{error::Truncated as TruncatedError, SafeBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Base128Error {
    #[error("Leading zero in base 128 integer")]
    LeadingZero,
    #[error("More than 5 bytes in base 128 integer")]
    MoreThan5Bytes,
    #[error("Truncated base 128 integer")]
    Truncated,
    #[error("Overflow in base 128 integer")]
    Overflow,
}

impl From<TruncatedError> for Base128Error {
    fn from(_: TruncatedError) -> Base128Error {
        Base128Error::Truncated
    }
}

pub trait BufExt {
    fn get_four_cc(&mut self) -> FourCC;
    fn try_get_four_cc(&mut self) -> Result<FourCC, TruncatedError>;
    fn try_get_base_128(&mut self) -> Result<u32, Base128Error>;
    fn try_get_255_u16(&mut self) -> Result<u16, TruncatedError>;
    fn try_copy_to_buf<T: BufMut>(
        &mut self,
        dest: &mut T,
        num_bytes: usize,
    ) -> Result<(), TruncatedError>;
    // vendored: bytes 1.6+ 给 `Buf` 加了 `try_get_*`（返回 `TryGetError`），
    // 与本 crate 期望的 `Truncated` 错误语义撞车且方法二义（E0034）。自建
    // 大端安全读，错误统一 `TruncatedError`。
    fn oc_take_u8(&mut self) -> Result<u8, TruncatedError>;
    fn oc_take_u16(&mut self) -> Result<u16, TruncatedError>;
    fn oc_take_u32(&mut self) -> Result<u32, TruncatedError>;
    fn oc_take_i16(&mut self) -> Result<i16, TruncatedError>;
}

impl<B> BufExt for B
where
    B: Buf,
{
    fn oc_take_u8(&mut self) -> Result<u8, TruncatedError> {
        if self.remaining() < 1 {
            return Err(TruncatedError);
        }
        Ok(self.get_u8())
    }

    fn oc_take_u16(&mut self) -> Result<u16, TruncatedError> {
        if self.remaining() < 2 {
            return Err(TruncatedError);
        }
        Ok(self.get_u16())
    }

    fn oc_take_u32(&mut self) -> Result<u32, TruncatedError> {
        if self.remaining() < 4 {
            return Err(TruncatedError);
        }
        Ok(self.get_u32())
    }

    fn oc_take_i16(&mut self) -> Result<i16, TruncatedError> {
        if self.remaining() < 2 {
            return Err(TruncatedError);
        }
        Ok(self.get_i16())
    }

    fn get_four_cc(&mut self) -> FourCC {
        let mut dest = [0; 4];
        self.copy_to_slice(&mut dest);
        FourCC(dest)
    }

    fn try_get_four_cc(&mut self) -> Result<FourCC, TruncatedError> {
        if self.remaining() < 4 {
            Err(TruncatedError)
        } else {
            Ok(self.get_four_cc())
        }
    }

    fn try_get_base_128(&mut self) -> Result<u32, Base128Error> {
        let mut accum = 0u32;
        for i in 0..5 {
            let byte = self.oc_take_u8()?;
            // no leading 0s
            if i == 0 && byte == 0x80 {
                return Err(Base128Error::LeadingZero);
            }
            // if any of the top seven bits are set, << 7 would overflow
            if accum >> 25 != 0 {
                return Err(Base128Error::Overflow);
            }
            accum = (accum << 7) | ((byte & 0x7F) as u32);
            if byte & 0x80 == 0 {
                return Ok(accum);
            }
        }
        Err(Base128Error::MoreThan5Bytes)
    }

    fn try_get_255_u16(&mut self) -> Result<u16, TruncatedError> {
        const ONE_MORE_BYTE_CODE_1: u8 = 255;
        const ONE_MORE_BYTE_CODE_2: u8 = 254;
        const WORD_CODE: u8 = 253;
        const LOWEST_UCODE: u16 = 253;
        let code = self.oc_take_u8()?;
        match code {
            WORD_CODE => self.oc_take_u16(),
            ONE_MORE_BYTE_CODE_1 => Ok(self.oc_take_u8()? as u16 + LOWEST_UCODE),
            ONE_MORE_BYTE_CODE_2 => Ok(self.oc_take_u8()? as u16 + 2 * LOWEST_UCODE),
            _ => Ok(code as u16),
        }
    }

    fn try_copy_to_buf<T: BufMut>(
        &mut self,
        dest: &mut T,
        mut num_bytes: usize,
    ) -> Result<(), TruncatedError> {
        if self.remaining() < num_bytes {
            return Err(TruncatedError);
        }
        loop {
            let chunk = self.chunk();
            if chunk.len() >= num_bytes {
                dest.put_slice(&chunk[..num_bytes]);
                self.advance(num_bytes);
                return Ok(());
            }
            let len = chunk.len();
            dest.put_slice(chunk);
            self.advance(len);
            num_bytes -= len;
        }
    }
}

/// Pads the buffer with zeros so its lenght is a multiple of four
pub fn pad_to_multiple_of_four(buffer: &mut Vec<u8>) {
    if buffer.len() & 3 != 0 {
        let new_len = (buffer.len() + 3) & !3;
        buffer.resize(new_len, 0);
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::BufExt;

    fn test_get_255_u16(expected: u16, data: &[u8]) {
        let mut buf = Cursor::new(data);
        let result = buf.try_get_255_u16();
        assert_eq!(expected, result.unwrap());
    }

    #[test]
    fn u255_uint_16_0_works() {
        test_get_255_u16(0, &[0u8]);
    }

    #[test]
    fn u255_uint_16_128_works() {
        test_get_255_u16(128, &[128u8]);
    }

    #[test]
    fn u255_uint_16_506_works() {
        test_get_255_u16(506, &[255, 253]);
        test_get_255_u16(506, &[254, 0]);
        test_get_255_u16(506, &[253, 1, 250]);
    }

    #[test]
    fn uint_base_128_0_works() {
        let mut buf = Cursor::new(&[0]);
        let result = buf.try_get_base_128();
        assert_eq!(0, result.unwrap());
    }

    #[test]
    fn uint_base_128_128_works() {
        let mut buf = Cursor::new(&[0x81u8, 0u8]);
        let result = buf.try_get_base_128();
        assert_eq!(128, result.unwrap());
    }

    #[test]
    fn try_copy_to_buf() {
        use bytes::{Buf, BufMut};
        let mut src: Cursor<&[u8]> = Cursor::new(&[42; 11]);
        let mut dest = Vec::new();

        src.try_copy_to_buf(&mut dest, 5).unwrap();
        assert_eq!(src.remaining(), 6);
        dest.put_u8(0);
        src.try_copy_to_buf(&mut dest, 5).unwrap();
        assert_eq!(src.remaining(), 1);

        assert!(src.try_copy_to_buf(&mut dest, 2).is_err());

        assert_eq!(dest, &[42, 42, 42, 42, 42, 0, 42, 42, 42, 42, 42]);
    }
}
