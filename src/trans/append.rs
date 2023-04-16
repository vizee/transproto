pub trait Append {
    fn append_into(self, buf: &mut Vec<u8>);
}

impl Append for f32 {
    fn append_into(self, buf: &mut Vec<u8>) {
        let mut b = dtoa::Buffer::new();
        let printed = b.format(self);
        buf.extend_from_slice(printed.as_bytes());
    }
}

impl Append for f64 {
    fn append_into(self, buf: &mut Vec<u8>) {
        let mut b = dtoa::Buffer::new();
        let printed = b.format(self);
        buf.extend_from_slice(printed.as_bytes());
    }
}

impl Append for bool {
    fn append_into(self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(if self { b"true" } else { b"false" });
    }
}

impl Append for i32 {
    fn append_into(self, buf: &mut Vec<u8>) {
        let mut b = itoa::Buffer::new();
        let printed = b.format(self);
        buf.extend_from_slice(printed.as_bytes());
    }
}

impl Append for i64 {
    fn append_into(self, buf: &mut Vec<u8>) {
        let mut b = itoa::Buffer::new();
        let printed = b.format(self);
        buf.extend_from_slice(printed.as_bytes());
    }
}

impl Append for u32 {
    fn append_into(self, buf: &mut Vec<u8>) {
        let mut b = itoa::Buffer::new();
        let printed = b.format(self);
        buf.extend_from_slice(printed.as_bytes());
    }
}

impl Append for u64 {
    fn append_into(self, buf: &mut Vec<u8>) {
        let mut b = itoa::Buffer::new();
        let printed = b.format(self);
        buf.extend_from_slice(printed.as_bytes());
    }
}
