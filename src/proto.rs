use std::{io, mem};

// https://developers.google.com/protocol-buffers/docs/encoding

const MAX_VARINT_SIZE: usize = 10;

pub const WIRE_VARINT: u32 = 0;
pub const WIRE_64BIT: u32 = 1;
pub const WIRE_LEN_DELIM: u32 = 2;
pub const WIRE_32BIT: u32 = 5;

#[inline]
pub fn proto_key(tag: u32, wire: u32) -> u64 {
    (tag as u64) << 3 | (wire as u64)
}

#[inline]
pub fn split_key(key: u64) -> (u32, u32) {
    ((key >> 3) as u32, (key & 7) as u32)
}

#[inline]
pub fn zigzag(v: i64) -> u64 {
    let u = (v as u64) << 1;
    if v >= 0 {
        u
    } else {
        !u
    }
}

#[inline]
pub fn unzigzag(u: u64) -> i64 {
    let v = (u >> 1) as i64;
    if u & 1 == 0 {
        v
    } else {
        !v
    }
}

pub struct Encoder {
    inner: Vec<u8>,
}

impl Encoder {
    pub const fn from_vec(buf: Vec<u8>) -> Self {
        Self { inner: buf }
    }

    pub const fn new() -> Self {
        Self::from_vec(Vec::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self::from_vec(Vec::with_capacity(capacity))
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.inner
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.inner.as_slice()
    }

    pub fn write_slice(&mut self, data: &[u8]) {
        self.inner.extend_from_slice(data);
    }

    pub fn write_varint(&mut self, v: u64) {
        if v >= 128 {
            unsafe {
                let mut buf: [u8; MAX_VARINT_SIZE] = mem::MaybeUninit::uninit().assume_init();
                let p = buf.as_mut_ptr();
                let mut n = 0usize;
                let mut v = v;
                loop {
                    p.add(n).write(0x80 | (v & 0x7f) as u8);
                    n += 1;
                    v >>= 7;
                    if v < 128 {
                        break;
                    }
                }
                p.add(n).write(v as u8);
                self.inner.extend_from_slice(&buf[..n + 1]);
            }
        } else {
            self.inner.push(v as u8);
        }
    }

    pub fn write_zigzag(&mut self, v: i64) {
        self.write_varint(zigzag(v));
    }

    pub fn emit_varint(&mut self, tag: u32, v: u64) {
        self.write_varint(proto_key(tag, WIRE_VARINT));
        self.write_varint(v);
    }

    pub fn emit_zigzag(&mut self, tag: u32, v: i64) {
        self.write_varint(proto_key(tag, WIRE_VARINT));
        self.write_zigzag(v);
    }

    pub fn emit_len_delim(&mut self, tag: u32, data: &[u8]) {
        self.write_varint(proto_key(tag, WIRE_LEN_DELIM));
        self.write_varint(data.len() as u64);
        self.write_slice(data);
    }

    pub fn emit_u64(&mut self, tag: u32, v: u64) {
        self.write_varint(proto_key(tag, WIRE_64BIT));
        self.write_slice(&v.to_le_bytes()[..]);
    }

    pub fn emit_u32(&mut self, tag: u32, v: u32) {
        self.write_varint(proto_key(tag, WIRE_32BIT));
        self.write_slice(&v.to_le_bytes()[..]);
    }
}

pub trait Fixed {
    type Raw: Copy;

    fn from_bytes(x: Self::Raw) -> Self;
}

impl Fixed for u32 {
    type Raw = [u8; 4];

    fn from_bytes(x: Self::Raw) -> Self {
        u32::from_le_bytes(x)
    }
}

impl Fixed for u64 {
    type Raw = [u8; 8];

    fn from_bytes(x: Self::Raw) -> Self {
        u64::from_le_bytes(x)
    }
}

impl Fixed for i32 {
    type Raw = [u8; 4];

    fn from_bytes(x: Self::Raw) -> Self {
        i32::from_le_bytes(x)
    }
}

impl Fixed for i64 {
    type Raw = [u8; 8];

    fn from_bytes(x: Self::Raw) -> Self {
        i64::from_le_bytes(x)
    }
}

impl Fixed for f32 {
    type Raw = [u8; 4];

    fn from_bytes(x: Self::Raw) -> Self {
        f32::from_le_bytes(x)
    }
}

impl Fixed for f64 {
    type Raw = [u8; 8];

    fn from_bytes(x: Self::Raw) -> Self {
        f64::from_le_bytes(x)
    }
}

pub struct Decoder<'a> {
    s: &'a [u8],
    p: usize,
}

impl Decoder<'_> {
    pub fn eof(&self) -> bool {
        self.s.len() == self.p
    }

    pub fn read_varint(&mut self) -> io::Result<u64> {
        let mut x = 0u64;
        let mut shift = 0u64;
        let mut i = self.p;
        while i < self.s.len() {
            let b = self.s[i];
            x |= ((b & 0x7f) as u64) << shift;
            if b < 0x80 {
                return if shift < 63 || (shift + b as u64 <= 64) {
                    self.p = i + 1;
                    Ok(x)
                } else {
                    Err(io::ErrorKind::InvalidData.into())
                };
            }
            shift += 7;
            i += 1;
        }
        Err(io::ErrorKind::UnexpectedEof.into())
    }

    pub fn read_key(&mut self) -> io::Result<(u32, u32)> {
        self.read_varint().map(split_key)
    }

    pub fn read_zigzag(&mut self) -> io::Result<i64> {
        self.read_varint().map(unzigzag)
    }

    pub fn read_fixed<T: Fixed>(&mut self) -> io::Result<T> {
        let n = mem::size_of::<T>();
        if self.s.len() - self.p >= n {
            Ok(unsafe {
                let p = self.s.as_ptr().add(self.p) as *const T::Raw;
                self.p += n;
                T::from_bytes(*p)
            })
        } else {
            Err(io::ErrorKind::UnexpectedEof.into())
        }
    }

    pub fn read_32bit(&mut self) -> io::Result<u32> {
        self.read_fixed::<u32>()
    }

    pub fn read_64bit(&mut self) -> io::Result<u64> {
        self.read_fixed::<u64>()
    }

    pub fn read_data(&mut self) -> io::Result<&[u8]> {
        self.read_varint().map(|x| x as usize).and_then(|n| {
            if self.p + n <= self.s.len() {
                let data = &self.s[self.p..self.p + n];
                self.p += n;
                Ok(data)
            } else {
                Err(io::ErrorKind::UnexpectedEof.into())
            }
        })
    }
}

impl<'a> Decoder<'a> {
    pub fn new(s: &'a [u8]) -> Self {
        Self { s, p: 0 }
    }
}

impl<'a> From<&'a [u8]> for Decoder<'a> {
    fn from(s: &'a [u8]) -> Self {
        Self::new(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proto_encode() {
        let mut enc = Encoder::new();
        enc.emit_varint(1, 233);
        enc.emit_len_delim(2, "test".as_bytes());
        enc.emit_u32(3, 987);
        enc.emit_zigzag(4, -233);
        assert_eq!(
            enc.as_bytes(),
            &[8u8, 233, 1, 18, 4, 116, 101, 115, 116, 29, 219, 3, 0, 0, 32, 209, 3]
        )
    }

    #[test]
    fn test_proto_decode() {
        let mut enc = Encoder::new();
        enc.emit_varint(1, 233);
        enc.emit_len_delim(2, "test".as_bytes());
        enc.emit_u32(3, 987);
        enc.emit_zigzag(4, -233);
        let mut dec = Decoder::new(enc.as_bytes());
        assert_eq!(dec.read_key().unwrap(), (1, WIRE_VARINT));
        assert_eq!(dec.read_varint().unwrap(), 233);
        assert_eq!(dec.read_key().unwrap(), (2, WIRE_LEN_DELIM));
        assert_eq!(dec.read_data().unwrap(), "test".as_bytes());
        assert_eq!(dec.read_key().unwrap(), (3, WIRE_32BIT));
        assert_eq!(dec.read_32bit().unwrap(), 987);
        assert_eq!(dec.read_key().unwrap(), (4, WIRE_VARINT));
        assert_eq!(dec.read_zigzag().unwrap(), -233);
    }

    #[test]
    fn test_proto_bad_decode() {
        let mut dec = Decoder::new(&[
            255, 255, 255, 255, 255, 255, 255, 255, 255, 0, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 1, 255, 255, 255, 255, 255, 255, 255, 255, 255, 2,
        ]);
        assert_eq!(dec.read_varint().ok(), Some(u64::MAX >> 1));
        assert_eq!(dec.read_varint().ok(), Some(u64::MAX));
        assert_eq!(
            dec.read_varint().err().unwrap().kind(),
            io::ErrorKind::InvalidData
        );
    }
}
