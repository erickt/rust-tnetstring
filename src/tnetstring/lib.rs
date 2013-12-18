#[link(name = "tnetstring",
       vers = "0.3",
       uuid = "ce93b70c-c22a-45fa-97a7-66ab97009005")];
#[crate_type = "lib"];

/// Rust TNetStrings serialization library.

use std::f64;
use std::hashmap::HashMap;
use std::io::Decorator;
use std::io::mem::MemWriter;
use std::num::strconv;
use std::str;
use std::vec;

/// Represents a TNetString value.
pub enum TNetString {
    Str(~[u8]),
    Int(int),
    Float(f64),
    Bool(bool),
    Null,
    Map(Map),
    Vec(~[TNetString]),
}

pub type Map = ~HashMap<~[u8], TNetString>;

/// Serializes a TNetString value into a `Writer`.
pub fn to_writer(writer: &mut Writer, tnetstring: &TNetString) {
    fn write_str(wr: &mut Writer, s: &[u8]) {
        write!(wr, "{}:", s.len());
        wr.write(s);
        write!(wr, ",");
    }

    match *tnetstring {
        Str(ref s) => {
            write_str(writer, *s);
        }
        Int(i) => {
            let s = i.to_str();
            write!(writer, "{}:{}\\#", s.len(), s);
        }
        Float(f) => {
            let s = f64::to_str_digits(f, 6u);
            write!(writer, "{}:{}^", s.len(), s);
        }
        Bool(b) => {
            let s = b.to_str();
            write!(writer, "{}:{}!", s.len(), s);
        }
        Map(ref m) => {
            let mut wr = MemWriter::new();
            for (key, value) in m.iter() {
                write_str(&mut wr as &mut Writer, *key);
                to_writer(&mut wr as &mut Writer, value);
            }
            let payload = wr.inner();
            write!(writer, "{}:", payload.len());
            writer.write(payload);
            write!(writer, "\\}");
        }
        Vec(ref v) => {
            let mut wr = MemWriter::new();
            for e in v.iter() {
                to_writer(&mut wr as &mut Writer, e)
            }
            let payload = wr.inner();
            write!(writer, "{}:", payload.len());
            writer.write(payload);
            write!(writer, "]");
        }
        Null => {
            write!(writer, "0:~");
        }
    }
}

/// Serializes a TNetString value into a byte string.
pub fn to_bytes(tnetstring: &TNetString) -> ~[u8] {
    let mut wr = MemWriter::new();
    to_writer(&mut wr as &mut Writer, tnetstring);
    wr.inner()
}

/// Serializes a TNetString value into a string.
impl ToStr for TNetString {
    fn to_str(&self) -> ~str {
        str::from_utf8_owned(to_bytes(self))
    }
}

/// Deserializes a TNetString value from an Iterator<u8>.
pub fn from_iter<T: Iterator<u8>>(iter: &mut T) -> Option<TNetString> {
    let mut ch = match iter.next() {
        Some(ch) => ch,
        None => { return None; }
    };
    let mut len = 0u;

    // Note that netstring spec explicitly forbids padding zeros.
    // If the first char is zero, it must be the only char.
    if ch < '0' as u8 || ch > '9' as u8 {
        fail!("Not a TNetString: invalid or missing length prefix");
    } else if ch == '0' as u8 {
        ch = match iter.next() {
            Some(ch) => ch,
            None => { fail!(); }
        };
    } else {
        loop {
            len = (10u * len) + ((ch as uint) - ('0' as uint));

            ch = match iter.next() {
                Some(ch) => ch,
                None => {
                    fail!("Not a TNetString: invalid or missing length prefix");
                }
            };

            if ch < '0' as u8 || ch > '9' as u8 {
                break;
            }
        }
    }

    // Validate end-of-length-prefix marker.
    if ch != ':' as u8 {
        fail!("Not a TNetString: missing length prefix");
    }

    // Read the data plus terminating type tag.
    let mut payload = vec::with_capacity(len);

    for _ in range(0, len) {
        match iter.next() {
            Some(ch) => { payload.push(ch); }
            None => { fail!("Not a TNetString: invalid length prefix"); }
        }
    }

    ch = match iter.next() {
        Some(ch) => ch,
        None => { fail!("Not a TNetString: missing type tag"); }
    };

    match ch as char {
        '#' => {
            let v = strconv::from_str_bytes_common(payload, 10, true, false, false,
                                                   strconv::ExpNone, false, false);
            v.and_then(|v| Some(Int(v)))
        }
        '}' => Some(Map(parse_map(payload))),
        ']' => Some(Vec(parse_vec(payload))),
        '!' => {
            str::from_utf8_owned_opt(payload)
                .and_then(|s| FromStr::from_str(s))
                .and_then(|v| Some(Bool(v)))
        }
        '^' => {
            let v = strconv::from_str_bytes_common(payload, 10u, true, true, true,
                                                   strconv::ExpDec, false, false);
            v.and_then(|v| Some(Float(v)))
        }
        '~' => {
            assert!(payload.is_empty());
            Some(Null)
        }
        ',' => {
            Some(Str(payload))
        }
        ch => {
            fail!(format!("Invalid payload type: {}", ch))
        }
    }
}

fn parse_vec(data: &[u8]) -> ~[TNetString] {
    if data.is_empty() { return ~[]; }

    let mut result = ~[];
    let mut iter = data.iter().map(|x| *x);

    loop {
        match from_iter(&mut iter) {
            Some(value) => { result.push(value); }
            None => { return result; }
        }
    }
}

fn parse_pair<T: Iterator<u8>>(iter: &mut T) -> Option<(~[u8], TNetString)> {
    match from_iter(iter) {
        Some(Str(key)) => {
            match from_iter(iter) {
                Some(value) => Some((key, value)),
                None => fail!("invalid TNetString"),
            }
        }
        Some(_) => fail!("Keys can only be strings."),
        None => None,
    }
}

fn parse_map(data: &[u8]) -> ~HashMap<~[u8], TNetString> {
    let mut result = ~HashMap::new();
    let mut iter = data.iter().map(|x| *x);

    loop {
        match parse_pair(&mut iter) {
            Some((key, value)) => { result.insert(key, value); }
            None => { return result; }
        }
    }
}

/// Deserializes a TNetString value from a byte string.
pub fn from_bytes(data: &[u8]) -> (Option<TNetString>, ~[u8]) {
    let mut iter = data.iter().map(|x| *x);
    let tnetstring = from_iter(&mut iter);
    (tnetstring, FromIterator::from_iterator(&mut iter))
}

/// Deserializes a TNetString value from a string.
pub fn from_str(data: &str) -> (Option<TNetString>, ~str) {
    let (tnetstring, bytes) = from_bytes(data.as_bytes());
    (tnetstring, str::from_utf8_owned(bytes))
}

/// Test the equality between two TNetString values
impl Eq for TNetString {
    fn eq(&self, other: &TNetString) -> bool {
        match (self, other) {
            (&Str(ref s0), &Str(ref s1)) => s0 == s1,
            (&Int(i0), &Int(i1)) => i0 == i1,
            (&Float(f0), &Float(f1)) => f0 == f1,
            (&Bool(b0), &Bool(b1)) => b0 == b1,
            (&Null, &Null) => true,
            (&Map(ref d0), &Map(ref d1)) => {
                if d0.len() == d1.len() {
                    for (k0, v0) in d0.iter() {
                        // XXX send_map::linear::LinearMap has find_ref, but
                        // that method is not available for HashMap.
                        let result = match d1.find(k0) {
                            Some(v1) => v0 == v1,
                            None => false,
                        };
                        if !result { return false; }
                    }
                    true
                } else {
                    false
                }
            }
            (&Vec(ref v0), &Vec(ref v1)) => {
                v0.eq(v1)
            },
            _ => false
        }
    }

    fn ne(&self, other: &TNetString) -> bool { !self.eq(other) }
}
