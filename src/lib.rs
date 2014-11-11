#![crate_name = "tnetstring"]

#![license = "MIT"]
#![crate_type = "dylib"]
#![crate_type = "rlib"]

/// Rust TNetStrings serialization library.

use std::collections::HashMap;
use std::f64;
use std::fmt;
use std::io;
use std::num::strconv;
use std::str;
use std::vec;

pub enum Error {
    MissingLengthPrefix,
    InvalidString,
    InvalidInteger,
    InvalidBool,
    InvalidFloat,
    InvalidNull,
    InvalidMap,
    InvalidPayloadType(u8),
    KeysCanOnlyBeStrings,
    IoError(io::IoError)
}

impl fmt::Show for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            MissingLengthPrefix => {
                write!(f, "missing length prefix")
            }
            InvalidString => {
                write!(f, "invalid string")
            }
            InvalidInteger => {
                write!(f, "invalid integer")
            }
            InvalidBool => {
                write!(f, "invalid bool")
            }
            InvalidFloat => {
                write!(f, "invalid float")
            }
            InvalidNull => {
                write!(f, "invalid null")
            }
            InvalidMap => {
                write!(f, "invalid map")
            }
            InvalidPayloadType(ch) => {
                write!(f, "invalid payload type '{}'", ch as char)
            }
            KeysCanOnlyBeStrings => {
                write!(f, "keys can only be strings")
            }
            IoError(ref err) => {
                err.fmt(f)
            }
        }
    }
}

/// Represents a TNetString value.
pub enum TNetString {
    Str(vec::Vec<u8>),
    Int(int),
    Float(f64),
    Bool(bool),
    Null,
    Map(HashMap<vec::Vec<u8>, TNetString>),
    Vec(vec::Vec<TNetString>),
}

/// Serializes a TNetString value into a `Writer`.
pub fn to_writer(writer: &mut Writer, tnetstring: &TNetString) -> io::IoResult<()> {
    fn write_str(wr: &mut Writer, s: &[u8]) -> io::IoResult<()> {
        try!(write!(wr, "{}:", s.len()));
        try!(wr.write(s));
        try!(write!(wr, ","));
        Ok(())
    }

    match *tnetstring {
        Str(ref s) => {
            write_str(writer, s.as_slice())
        }
        Int(i) => {
            let s = i.to_string();
            write!(writer, "{}:{}#", s.len(), s)
        }
        Float(f) => {
            let s = f64::to_str_digits(f, 6u);
            write!(writer, "{}:{}^", s.len(), s)
        }
        Bool(b) => {
            let s = b.to_string();
            write!(writer, "{}:{}!", s.len(), s)
        }
        Map(ref m) => {
            let mut wr = io::MemWriter::new();
            for (key, value) in m.iter() {
                try!(write_str(&mut wr as &mut Writer, key.as_slice()));
                try!(to_writer(&mut wr as &mut Writer, value));
            }
            let payload = wr.unwrap();
            try!(write!(writer, "{}:", payload.len()));
            try!(writer.write(payload.as_slice()));
            write!(writer, "}}")
        }
        Vec(ref v) => {
            let mut wr = io::MemWriter::new();
            for e in v.iter() {
                try!(to_writer(&mut wr as &mut Writer, e))
            }
            let payload = wr.unwrap();
            try!(write!(writer, "{}:", payload.len()));
            try!(writer.write(payload.as_slice()));
            write!(writer, "]")
        }
        Null => {
            write!(writer, "0:~")
        }
    }
}

/// Serializes a TNetString value into a byte string.
pub fn to_bytes(tnetstring: &TNetString) -> io::IoResult<vec::Vec<u8>> {
    let mut wr = io::MemWriter::new();
    try!(to_writer(&mut wr as &mut Writer, tnetstring));
    Ok(wr.unwrap())
}

/// Serializes a TNetString value into a string.
impl fmt::Show for TNetString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        to_writer(f, self).map_err(|_| fmt::WriteError)
    }
}

/// Deserializes a `TNetString` value from a `Reader`.
pub fn from_reader<R: Reader + Buffer>(rdr: &mut R) -> Result<Option<TNetString>, Error> {
    let mut ch = match rdr.read_byte() {
        Ok(ch) => ch,
        Err(ref err) if err.kind == io::EndOfFile => { return Ok(None); }
        Err(err) => { return Err(IoError(err)); }
    };
    let mut len = 0u;

    // Note that netstring spec explicitly forbids padding zeros.
    // If the first char is zero, it must be the only char.
    if ch < '0' as u8 || ch > '9' as u8 {
        return Err(MissingLengthPrefix);
    } else if ch == '0' as u8 {
        ch = match rdr.read_byte() {
            Ok(ch) => ch,
            Err(err) => { return Err(IoError(err)); }
        };
    } else {
        loop {
            len = (10u * len) + ((ch as uint) - ('0' as uint));

            ch = match rdr.read_byte() {
                Ok(ch) => ch,
                Err(err) => { return Err(IoError(err)); }
            };

            if ch < '0' as u8 || ch > '9' as u8 {
                break;
            }
        }
    }

    // Validate end-of-length-prefix marker.
    if ch != ':' as u8 {
        return Err(MissingLengthPrefix);
    }

    // Read the data plus terminating type tag.
    let payload = match rdr.read_exact(len) {
        Ok(payload) => payload,
        Err(err) => { return Err(IoError(err)); }
    };

    if payload.len() != len {
        return Err(MissingLengthPrefix);
    }

    let ch = match rdr.read_byte() {
        Ok(ch) => ch,
        Err(err) => { return Err(IoError(err)); }
    };

    let value = match ch {
        b'#' => {
            let payload = match str::from_utf8(payload.as_slice()) {
                Some(payload) => payload,
                None => { return Err(InvalidString); }
            };

            match strconv::from_str_radix_int(payload, 10) {
                Some(v) => Some(Int(v)),
                None => { return Err(InvalidInteger); }
            }
        }
        b'}' => Some(Map(try!(parse_map(payload.as_slice())))),
        b']' => Some(Vec(try!(parse_vec(payload.as_slice())))),
        b'!' => {
            match payload.as_slice() {
                b"true" => Some(Bool(true)),
                b"false" => Some(Bool(false)),
                _ => { return Err(InvalidBool); }
            }
        }
        b'^' => {
            let payload = match str::from_utf8(payload.as_slice()) {
                Some(payload) => payload,
                None => { return Err(InvalidString); }
            };

            match strconv::from_str_radix_float(payload, 10) {
                Some(v) => Some(Float(v)),
                None => { return Err(InvalidFloat); }
            }
        }
        b'~' => {
            if payload.is_empty() {
                Some(Null)
            } else {
                return Err(InvalidNull);
            }
        }
        b',' => {
            Some(Str(payload))
        }
        ch => {
            return Err(InvalidPayloadType(ch));
        }
    };

    Ok(value)
}

fn parse_vec(data: &[u8]) -> Result<vec::Vec<TNetString>, Error> {
    if data.is_empty() { return Ok(vec![]); }

    let mut result = vec![];
    let mut rdr = io::BufReader::new(data);

    loop {
        match try!(from_reader(&mut rdr)) {
            Some(value) => { result.push(value); }
            None => { return Ok(result); }
        }
    }
}

fn parse_pair<R: Reader + Buffer>(rdr: &mut R) -> Result<Option<(vec::Vec<u8>, TNetString)>, Error> {
    match try!(from_reader(rdr)) {
        Some(Str(key)) => {
            match try!(from_reader(rdr)) {
                Some(value) => Ok(Some((key, value))),
                None => { return Err(InvalidMap); }
            }
        }
        Some(_) => Err(KeysCanOnlyBeStrings),
        None => Ok(None),
    }
}

fn parse_map(data: &[u8]) -> Result<HashMap<vec::Vec<u8>, TNetString>, Error> {
    let mut result = HashMap::new();
    let mut rdr = io::BufReader::new(data);

    loop {
        match try!(parse_pair(&mut rdr)) {
            Some((key, value)) => { result.insert(key, value); }
            None => { return Ok(result); }
        }
    }
}

/// Deserializes a TNetString value from a byte string.
pub fn from_bytes<'a>(data: &'a [u8]) -> Result<(Option<TNetString>, io::BufReader<'a>), Error> {
    let mut rdr = io::BufReader::new(data);
    let tnetstring = try!(from_reader(&mut rdr));
    Ok((tnetstring, rdr))
}

/// Deserializes a TNetString value from a string.
pub fn from_str<'a>(data: &'a str) -> Result<(Option<TNetString>, io::BufReader<'a>), Error> {
    from_bytes(data.as_bytes())
}

/// Test the equality between two TNetString values
impl PartialEq for TNetString {
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
                        let result = match d1.get(k0) {
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::f64;
    use std::from_str::FromStr;
    use std::rand::Rng;
    use std::rand;
    use std::vec;

    use super::TNetString;
    use super::{Int, Float, Bool, Null, Map, Vec};
    use super::{from_bytes, to_bytes};
    use super::from_str;

    // Tests inspired by https://github.com/rfk/TNetString.

    fn test(s: &str, expected: &TNetString) {
        let (actual, rest) = from_str(s).unwrap();
        assert!(actual.is_some());
        assert!(rest.eof());

        let actual = actual.unwrap();
        assert_eq!(actual, *expected);
        assert_eq!(expected.to_string(), s.to_string());
    }

    #[test]
    fn test_format() {
        test("11:hello world,", &super::Str(b"hello world".to_vec()));
        test("0:}", &Map(HashMap::new()));
        test("0:]", &Vec(vec![]));

        let mut d = HashMap::new();
        d.insert(b"hello".to_vec(),
                Vec(vec![
                    Int(12345678901),
                    super::Str(b"this".to_vec()),
                    Bool(true),
                    Null,
                    super::Str(b"\x00\x00\x00\x00".to_vec())
                ]));

        test("51:5:hello,39:11:12345678901#4:this,4:true!0:~4:\x00\x00\x00\
               \x00,]}", &Map(d));

        test("5:12345#", &Int(12345));
        test("12:this is cool,", &super::Str(b"this is cool".to_vec()));
        test("0:,", &super::Str(b"".to_vec()));
        test("0:~", &Null);
        test("4:true!", &Bool(true));
        test("5:false!", &Bool(false));
        test("10:\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00,",
            &super::Str(b"\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00".to_vec()));
        test("24:5:12345#5:67890#5:xxxxx,]",
            &Vec(vec![
                Int(12345),
                Int(67890),
                super::Str(b"xxxxx".to_vec())]));
        test("18:3:0.1^3:0.2^3:0.4^]",
           &Vec(vec![Float(0.1), Float(0.2), Float(0.4)]));
        test("243:238:233:228:223:218:213:208:203:198:193:188:183:178:173:\
               168:163:158:153:148:143:138:133:128:123:118:113:108:103:99:95:\
               91:87:83:79:75:71:67:63:59:55:51:47:43:39:35:31:27:23:19:15:\
               11:hello-there,]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]\
               ]]]]",
            &Vec(
                vec![Vec(vec![Vec(vec![Vec(vec![Vec(vec![Vec(vec![Vec(vec![Vec(vec![Vec(
                vec![Vec(vec![Vec(vec![Vec(vec![Vec(vec![Vec(vec![Vec(vec![Vec(vec![Vec(
                vec![Vec(vec![Vec(vec![Vec(vec![Vec(vec![Vec(vec![Vec(vec![Vec(vec![Vec(
                vec![Vec(vec![Vec(vec![Vec(vec![Vec(vec![Vec(vec![Vec(vec![Vec(vec![Vec(
                vec![Vec(vec![Vec(vec![Vec(vec![Vec(vec![Vec(vec![Vec(vec![Vec(vec![Vec(
                vec![Vec(vec![Vec(vec![Vec(vec![Vec(vec![Vec(vec![Vec(vec![Vec(vec![Vec(
                vec![Vec(vec![Vec(vec![
                    super::Str(b"hello-there".to_vec())
                ])])])])])])])])])])])])])])])])])])])])])])])])])])])])
                ])])])])])])])])])])])])])])])])])])])])])])]));
    }

    #[test]
    fn test_random() {
        fn get_random_object<T: Rng>(rng: &mut T, depth: u32) -> TNetString {
            if rng.gen_range(depth, 10u32) <= 4u32 {
                if rng.gen_range(0u32, 1u32) == 0u32 {
                    let n = rng.gen_range(0u32, 10u32);
                    Vec(vec::Vec::from_fn(n as uint, |_i|
                        get_random_object(rng, depth + 1u32)
                    ))
                } else {
                    let mut d = HashMap::new();

                    let mut i = rng.gen_range(0u32, 10u32);
                    while i != 0u32 {
                        let n = rng.gen_range(0u32, 100u32) as uint;
                        let s = rng.gen_iter::<u8>().take(n).collect();
                        d.insert(
                            s,
                            get_random_object(rng, depth + 1u32)
                        );
                        i -= 1u32;
                    }
                    Map(d)
                }
            } else {
                match rng.gen_range(0u32, 5u32) {
                  0u32 => Null,
                  1u32 => Bool(true),
                  2u32 => Bool(false),
                  3u32 => Int(rng.gen()),
                  4u32 => {
                    let mut f = rng.gen::<f64>();

                    // Generate a float that can be exactly converted to
                    // and from a string.
                    loop {
                        match FromStr::from_str(f64::to_str_digits(f, 6).as_slice()) {
                          Some(f1) => {
                            if f == f1 { break; }
                            f = f1;
                          }
                          None => panic!("invalid float")
                        }
                    }

                    if rng.gen_range(0u32, 1u32) == 0u32 {
                        Float(f)
                    } else {
                        Float(-f)
                    }
                  }
                  5u32 => {
                    let n = rng.gen_range(0u32, 100u32) as uint;
                    super::Str(rng.gen_iter::<u8>().take(n).collect())
                  }
                  _ => panic!()
                }
            }
        }

        let mut rng = rand::task_rng();

        let mut i = 500;
        while i != 0 {
            let v0 = get_random_object(&mut rng, 0u32);
            let bytes = to_bytes(&v0).unwrap();

            match from_bytes(bytes.as_slice()).unwrap() {
                (Some(ref v1), rest) if rest.eof() => {
                    assert!(v0 == *v1)
                },
                _ => panic!("invalid TNetString")
            }
            i -= 1u;
        }
    }
}
