#[link(name = "tnetstring",
       vers = "0.3",
       uuid = "ce93b70c-c22a-45fa-97a7-66ab97009005")];
#[crate_type = "lib"];

/// Rust TNetStrings serialization library.

use std::hashmap::HashMap;
use std::io;
use std::str;
use std::f64;

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

/// Serializes a TNetString value into a io::Writer.
pub fn to_writer(writer: @io::Writer, tnetstring: &TNetString) {
    fn write_str(wr: @io::Writer, s: &[u8]) {
        wr.write_str(format!("{}:", s.len()));
        wr.write(s);
        wr.write_char(',');
    }

    match *tnetstring {
        Str(ref s) => write_str(writer, *s),
        Int(i) => {
            let s = i.to_str();
            writer.write_str(format!("{}:{}\\#", s.len(), s));
        }
        Float(f) => {
            let s = f64::to_str_digits(f, 6u);
            writer.write_str(format!("{}:{}^", s.len(), s));
        }
        Bool(b) => {
            let s = b.to_str();
            writer.write_str(format!("{}:{}!", s.len(), s));
        }
        Map(ref m) => {
            let payload = do io::with_bytes_writer |wr| {
                for (key, value) in m.iter() {
                    write_str(wr, *key);
                    to_writer(wr, value);
                }
            };
            writer.write_str(format!("{}:", payload.len()));
            writer.write(payload);
            writer.write_char('}');
        }
        Vec(ref v) => {
            let payload = do io::with_bytes_writer |wr| {
                for e in v.iter() {
                    to_writer(wr, e)
                }
            };
            writer.write_str(format!("{}:", payload.len()));
            writer.write(payload);
            writer.write_char(']');
        }
        Null => writer.write_str("0:~"),
    }
}

/// Serializes a TNetString value into a byte string.
pub fn to_bytes(tnetstring: &TNetString) -> ~[u8] {
    do io::with_bytes_writer |wr| {
        to_writer(wr, tnetstring);
    }
}

/// Serializes a TNetString value into a string.
impl ToStr for TNetString {
    fn to_str(&self) -> ~str {
        do io::with_str_writer |wr| {
            to_writer(wr, self);
        }
    }
}

/// Deserializes a TNetString value from an io::Reader.
pub fn from_reader(reader: @io::Reader) -> Option<TNetString> {
    assert!(!reader.eof());

    let mut c = reader.read_byte();
    let mut len = 0u;

    // Note that netstring spec explicitly forbids padding zeros.
    // If the first char is zero, it must be the only char.
    if c < '0' as int || c > '9' as int {
        fail!("Not a TNetString: invalid or missing length prefix");
    } else if c == '0' as int {
        c = reader.read_byte();
    } else {
        loop {
            len = (10u * len) + ((c as uint) - ('0' as uint));

            if reader.eof() {
                fail!("Not a TNetString: invalid or missing length prefix");
            }
            c = reader.read_byte();

            if c < '0' as int || c > '9' as int {
                break;
            }
        }
    }

    // Validate end-of-length-prefix marker.
    if c != ':' as int {
        fail!("Not a TNetString: missing length prefix");
    }

    // Read the data plus terminating type tag.
    let payload = reader.read_bytes(len);

    if payload.len() != len {
        fail!("Not a TNetString: invalid length prefix");
    }

    if reader.eof() {
        fail!("Not a TNetString: missing type tag");
    }

    match reader.read_char() {
        '#' => {
            let s = unsafe { str::raw::from_utf8(payload) };
            FromStr::from_str(s).and_then(|v| Some(Int(v)))
        }
        '}' => Some(Map(parse_map(payload))),
        ']' => Some(Vec(parse_vec(payload))),
        '!' => {
            let s = unsafe { str::raw::from_utf8(payload) };
            FromStr::from_str(s).and_then(|v| Some(Bool(v)))
        }
        '^' => {
            let s = unsafe { str::raw::from_utf8(payload) };
            FromStr::from_str(s).and_then(|v| Some(Float(v)))
        }
        '~' => {
            assert!(payload.is_empty());
            Some(Null)
        }
        ',' => Some(Str(payload)),
        c => {
            fail!(format!("Invalid payload type: {}", c))
        }
    }
}

fn parse_vec(data: &[u8]) -> ~[TNetString] {
    if data.len() == 0u { return ~[]; }

    do io::with_bytes_reader(data) |reader| {
        let mut result = ~[];

        match from_reader(reader) {
            Some(value) => result.push(value),
            None => fail!("invalid value")
        }

        while !reader.eof() {
            match from_reader(reader) {
                Some(value) => result.push(value),
                None => fail!("invalid TNetString")
            }
        }

        result
    }
}

fn parse_pair(reader: @io::Reader) -> (~[u8], TNetString) {
    match from_reader(reader) {
        Some(Str(key)) => {
            match from_reader(reader) {
                Some(value) => (key, value),
                None => fail!("invalid TNetString"),
            }
        }
        Some(_) => fail!("Keys can only be strings."),
        None => fail!("Invalid TNetString"),
    }
}

fn parse_map(data: &[u8]) -> ~HashMap<~[u8], TNetString> {
    let mut result = ~HashMap::new();

    if data.len() != 0u {
        do io::with_bytes_reader(data) |reader| {
            let (key, value) = parse_pair(reader);
            result.insert(key, value);

            while !reader.eof() {
                let (key, value) = parse_pair(reader);
                result.insert(key, value);
            }
        }
    }

    result
}

/// Deserializes a TNetString value from a byte string.
pub fn from_bytes(data: &[u8]) -> (Option<TNetString>, ~[u8]) {
    do io::with_bytes_reader(data) |reader| {
        let tnetstring = from_reader(reader);
        (tnetstring, reader.read_whole_stream())
    }
}

/// Deserializes a TNetString value from a string.
pub fn from_str(data: &str) -> (Option<TNetString>, ~str) {
    do io::with_str_reader(data) |rdr| {
        let tnetstring = from_reader(rdr);
        let bytes = rdr.read_whole_stream();
        (tnetstring, str::from_utf8(bytes))
    }

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

#[cfg(test)]
mod tests {
    use std::f64;
    use std::hashmap::HashMap;
    use std::rand;
    use std::rand::Rng;
    use std::vec;

    use super::TNetString;
    use super::{Str, Int, Float, Bool, Null, Map, Vec};
    use super::{from_bytes, to_bytes};
    use super::from_str;

    // Tests inspired by https://github.com/rfk/TNetString.

    fn test(s: &str, expected: &TNetString) {
        let (actual, rest) = from_str(s);
        assert!(actual.is_some());
        assert!(rest.is_empty());

        let actual = actual.unwrap();
        assert_eq!(actual, *expected);
        assert_eq!(expected.to_str(), s.to_owned());
    }

    #[test]
    fn test_format() {
        test("11:hello world,", &Str((~"hello world").into_bytes()));
        test("0:}", &Map(~HashMap::new()));
        test("0:]", &Vec(~[]));

        let mut d = ~HashMap::new();
        d.insert((~"hello").into_bytes(),
                Vec(~[
                    Int(12345678901),
                    Str((~"this").into_bytes()),
                    Bool(true),
                    Null,
                    Str((~"\x00\x00\x00\x00").into_bytes())
                ]));

        test("51:5:hello,39:11:12345678901#4:this,4:true!0:~4:\x00\x00\x00\
               \x00,]}", &Map(d));

        test("5:12345#", &Int(12345));
        test("12:this is cool,", &Str((~"this is cool").into_bytes()));
        test("0:,", &Str((~"").into_bytes()));
        test("0:~", &Null);
        test("4:true!", &Bool(true));
        test("5:false!", &Bool(false));
        test("10:\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00,",
            &Str((~"\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00").into_bytes()));
        test("24:5:12345#5:67890#5:xxxxx,]",
            &Vec(~[
                Int(12345),
                Int(67890),
                Str((~"xxxxx").into_bytes())]));
        test("18:3:0.1^3:0.2^3:0.4^]",
           &Vec(~[Float(0.1), Float(0.2), Float(0.4)]));
        test("243:238:233:228:223:218:213:208:203:198:193:188:183:178:173:\
               168:163:158:153:148:143:138:133:128:123:118:113:108:103:99:95:\
               91:87:83:79:75:71:67:63:59:55:51:47:43:39:35:31:27:23:19:15:\
               11:hello-there,]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]\
               ]]]]",
            &Vec(
                ~[Vec(~[Vec(~[Vec(~[Vec(~[Vec(~[Vec(~[Vec(~[Vec(
                ~[Vec(~[Vec(~[Vec(~[Vec(~[Vec(~[Vec(~[Vec(~[Vec(
                ~[Vec(~[Vec(~[Vec(~[Vec(~[Vec(~[Vec(~[Vec(~[Vec(
                ~[Vec(~[Vec(~[Vec(~[Vec(~[Vec(~[Vec(~[Vec(~[Vec(
                ~[Vec(~[Vec(~[Vec(~[Vec(~[Vec(~[Vec(~[Vec(~[Vec(
                ~[Vec(~[Vec(~[Vec(~[Vec(~[Vec(~[Vec(~[Vec(~[Vec(
                ~[Vec(~[Vec(~[
                    Str((~"hello-there").into_bytes())
                ])])])])])])])])])])])])])])])])])])])])])])])])])])])])
                ])])])])])])])])])])])])])])])])])])])])])])]));
    }

    #[test]
    fn test_random() {
        fn get_random_object(rng: &mut rand::StdRng, depth: u32) -> TNetString {
            if rng.gen_integer_range(depth, 10u32) <= 4u32 {
                if rng.gen_integer_range(0u32, 1u32) == 0u32 {
                    let n = rng.gen_integer_range(0u32, 10u32);
                    Vec(vec::from_fn(n as uint, |_i|
                        get_random_object(rng, depth + 1u32)
                    ))
                } else {
                    let mut d = ~HashMap::new();

                    let mut i = rng.gen_integer_range(0u32, 10u32);
                    while i != 0u32 {
                        let n = rng.gen_integer_range(0u32, 100u32) as uint;
                        let s = rng.gen_vec(n);
                        d.insert(
                            s,
                            get_random_object(rng, depth + 1u32)
                        );
                        i -= 1u32;
                    }
                    Map(d)
                }
            } else {
                match rng.gen_integer_range(0u32, 5u32) {
                  0u32 => Null,
                  1u32 => Bool(true),
                  2u32 => Bool(false),
                  3u32 => {
                    if rng.gen_integer_range(0u32, 1u32) == 0u32 {
                        Int(rng.next_u32() as int)
                    } else {
                        Int(-rng.next_u32() as int)
                    }
                  }
                  4u32 => {
                    let mut f = rng.gen::<f64>();

                    // Generate a float that can be exactly converted to
                    // and from a string.
                    loop {
                        match FromStr::from_str(f64::to_str_digits(f, 6)) {
                          Some(f1) => {
                            if f == f1 { break; }
                            f = f1;
                          }
                          None => fail!("invalid float")
                        }
                    }

                    if rng.gen_integer_range(0u32, 1u32) == 0u32 {
                        Float(f)
                    } else {
                        Float(-f)
                    }
                  }
                  5u32 => {
                    let n = rng.gen_integer_range(0u32, 100u32) as uint;
                    Str(rng.gen_vec(n))
                  }
                  _ => fail!()
                }
            }
        }

        let mut rng = rand::rng();

        let mut i = 500u;
        while i != 0u {
            let v0 = get_random_object(&mut rng, 0u32);

            match from_bytes(to_bytes(&v0)) {
                (Some(ref v1), ref rest) if *rest == ~[] => {
                    assert!(v0 == *v1)
                },
                _ => fail!("invalid TNetString")
            }
            i -= 1u;
        }
    }
}
