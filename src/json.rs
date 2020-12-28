#[derive(Debug)]
pub enum Token<'a> {
    Invalid(&'a [u8]),
    Null,
    False,
    True,
    Number(&'a [u8]),
    String(&'a [u8]),
    Comma,
    Colon,
    Object,
    ObjectClose,
    Array,
    ArrayClose,
}

pub struct Iter<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Iter<'a> {
    pub fn new(s: &'a [u8]) -> Self {
        Self { s, i: 0 }
    }

    #[inline]
    pub fn eof(&self) -> bool {
        self.i >= self.s.len()
    }

    pub fn reset(&mut self, s: &'a [u8]) {
        self.s = s;
        self.i = 0;
    }

    fn skip_whitespace(&mut self) {
        let mut i = self.i;
        while let Some(c) = self.s.get(i) {
            if !c.is_ascii_whitespace() {
                break;
            }
            i += 1;
        }
        self.i = i;
    }

    fn read_string(&mut self) -> Token<'a> {
        let b = self.i;
        let mut i = b + 1;
        while i < self.s.len() {
            if self.s[i] == b'"' && self.s[i - 1] != b'\\' {
                self.i = i + 1;
                return Token::String(&self.s[b..self.i]);
            }
            i += 1;
        }
        Token::Invalid(&self.s[b..])
    }

    fn read_number(&mut self) -> Token<'a> {
        let b = self.i;
        let mut i = b + 1;
        while let Some(c) = self.s.get(i) {
            match c {
                b'0'..=b'9' | b'.' | b'-' | b'e' | b'E' => i += 1,
                _ => break,
            }
        }
        self.i = i;
        Token::Number(&self.s[b..i])
    }

    #[inline]
    fn consume(&mut self, tok: Token<'a>) -> Token<'a> {
        self.i += 1;
        tok
    }

    fn fixed(&mut self, expected: usize, tok: Token<'a>) -> Token<'a> {
        if self.i + expected > self.s.len() {
            Token::Invalid(&self.s[self.i..])
        } else {
            self.i += expected;
            tok
        }
    }

    fn next_token(&mut self) -> Option<Token<'a>> {
        self.skip_whitespace();
        self.s.get(self.i).map(|&c| match c {
            b'n' => self.fixed(4, Token::Null),
            b't' => self.fixed(4, Token::True),
            b'f' => self.fixed(5, Token::False),
            b'{' => self.consume(Token::Object),
            b'}' => self.consume(Token::ObjectClose),
            b'[' => self.consume(Token::Array),
            b']' => self.consume(Token::ArrayClose),
            b',' => self.consume(Token::Comma),
            b':' => self.consume(Token::Colon),
            b'"' => self.read_string(),
            _ if c.is_ascii_digit() || c == b'-' => self.read_number(),
            _ => Token::Invalid(&self.s[self.i..self.i + 1]),
        })
    }
}

impl<'a> From<&'a [u8]> for Iter<'a> {
    fn from(s: &'a [u8]) -> Self {
        Self::new(s)
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = Token<'a>;

    fn next(&mut self) -> Option<Token<'a>> {
        self.next_token()
    }
}

const RAW_MARK: u8 = b'0';

const ESCAPE_TABLE: &[u8] = b"00000000btn0fr00000000000000000000\"000000000000/00000000000000000000000000000000000000000000\\";

pub fn escape_string(s: &[u8], z: &mut Vec<u8>) {
    let mut last = 0usize;
    let mut i = 0usize;
    while i < s.len() {
        let c = s[i];
        // todo: validate utf-8 string
        if c as usize >= ESCAPE_TABLE.len() || ESCAPE_TABLE[c as usize] == RAW_MARK {
            i += 1;
            continue;
        }
        if last < i {
            z.extend_from_slice(&s[last..i]);
        }
        z.push(b'\\');
        z.push(ESCAPE_TABLE[c as usize]);
        i += 1;
        last = i;
    }
    if last < s.len() {
        z.extend_from_slice(&s[last..]);
    }
}

const UNESCAPE_TABLE: &[u8] = b"0000000000000000000000000000000000\"000000000000/00000000000000000000000000000000000000000000\\00000\x08000\x0C0000000\n000\r0\tu";

pub fn unescape_string(s: &[u8], z: &mut Vec<u8>) -> Result<(), String> {
    let mut i = 0usize;
    while i < s.len() {
        let c = s[i];
        if c == b'\\' {
            i += 1;
            let c = s[i];
            if c as usize >= UNESCAPE_TABLE.len() || UNESCAPE_TABLE[c as usize] == RAW_MARK {
                return Err(format!("invalid escape character: '{}'", c as char));
            }
            if c == b'u' {
                if i + 4 >= s.len() {
                    return Err("invalid escape character".to_string());
                }
                let mut uc = 0u32;
                for _ in 0..4 {
                    i += 1;
                    let c = s[i];
                    match c {
                        b'0'..=b'9' => uc = uc << 4 | (c - b'0') as u32,
                        b'A'..=b'F' => uc = uc << 4 | (c - b'A' + 10) as u32,
                        b'a'..=b'f' => uc = uc << 4 | (c - b'a' + 10) as u32,
                        _ => {
                            return Err(format!("invalid unicode escape sequence: '{}'", c as char))
                        }
                    }
                }
                if let Some(c) = ::std::char::from_u32(uc) {
                    let mut dst = [0u8; 6];
                    let cs = c.encode_utf8(&mut dst);
                    z.extend_from_slice(cs.as_bytes());
                } else {
                    return Err(format!("invalid unicode character: '{:x}'", uc));
                }
            } else {
                z.push(UNESCAPE_TABLE[c as usize]);
            }
        } else {
            z.push(c);
        }
        i += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json() {
        let s = b"{\"animals\":{\"dog\":[{\"name\":\"Rufus\",\"age\":15},{\"name\":\"Marty\",\"age\":null}]}}";
        for t in Iter::new(s) {
            println!("{:?}", t);
            assert!(!matches!(t, Token::Invalid(_)));
        }
    }

    #[test]
    fn test_escape_string() {
        let mut o1 = Vec::new();
        escape_string(b"\t", &mut o1);
        assert_eq!(String::from_utf8(o1).unwrap(), "\\t");
        let mut o2 = Vec::new();
        escape_string(b"123\tabc", &mut o2);
        assert_eq!(String::from_utf8(o2).unwrap(), "123\\tabc");
    }

    #[test]
    fn test_unescape_string() {
        let mut o1 = Vec::new();
        unescape_string(b"123\\tabc", &mut o1).unwrap();
        assert_eq!(String::from_utf8(o1).unwrap(), "123\tabc");
        let mut o2 = Vec::new();
        unescape_string(b"\\u4f60\\u597d", &mut o2).unwrap();
        assert_eq!(String::from_utf8(o2).unwrap(), "你好");
    }
}
