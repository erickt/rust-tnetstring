/// Rust TNetStrings serialization library.

export tnetstring;
export str;
export int;
export float;
export bool;
export null;
export map;
export vec;
export to_writer;
export to_bytes;
export to_str;
export from_reader;
export from_bytes;
export from_str;

/// Represents a tnetstring value.
enum tnetstring {
    str(~[u8]),
    int(int),
    float(float),
    bool(bool),
    null,
    map(~send_map::linear::LinearMap<~[u8], tnetstring>),
    vec(~[tnetstring]),
}

/// Serializes a tnetstring value into a io::Writer.
fn to_writer(writer: io::Writer, tnetstring: tnetstring) {
    fn write_str(wr: io::Writer, s: &[u8]) {
        wr.write_str(#fmt("%u:", s.len()));
        wr.write(s);
        wr.write_char(',');
    }

    match tnetstring {
        str(s) => write_str(writer, s),
        int(i) => {
            let s = int::str(i);
            writer.write_str(#fmt("%u:%s#", s.len(), s));
        }
        float(f) => {
            let s = float::to_str(f, 6u);
            writer.write_str(#fmt("%u:%s^", s.len(), s));
        }
        bool(b) => {
            let s = bool::to_str(b);
            writer.write_str(#fmt("%u:%s!", s.len(), s));
        }
        map(m) => {
            let buf = io::mem_buffer();
            let wr = io::mem_buffer_writer(buf);
            for m.each_ref |key, value| {
                write_str(wr, *key);
                to_writer(wr, *value);
            }
            let payload = io::mem_buffer_buf(buf);
            writer.write_str(#fmt("%u:", payload.len()));
            writer.write(payload);
            writer.write_char('}');
        }
        vec(v) => {
            let buf = io::mem_buffer();
            let wr = io::mem_buffer_writer(buf);
            for v.each |e| { to_writer(wr, e) }
            let payload = io::mem_buffer_buf(buf);
            writer.write_str(#fmt("%u:", payload.len()));
            writer.write(payload);
            writer.write_char(']');
        }
        null => writer.write_str("0:~"),
    }
}

/// Serializes a tnetstring value into a byte string.
fn to_bytes(tnetstring: tnetstring) -> ~[u8] {
    let buf = io::mem_buffer();
    let wr = io::mem_buffer_writer(buf);
    to_writer(wr, tnetstring);
    io::mem_buffer_buf(buf)
}

/// Serializes a tnetstring value into a string.
fn to_str(tnetstring: tnetstring) -> ~str {
    let buf = io::mem_buffer();
    let wr = io::mem_buffer_writer(buf);
    to_writer(wr, tnetstring);
    io::mem_buffer_str(buf)
}

/// Deserializes a tnetstring value from an io::Reader.
fn from_reader(reader: io::Reader) -> Option<tnetstring> {
    assert !reader.eof();

    let mut c = reader.read_byte();
    let mut len = 0u;

    // Note that netstring spec explicitly forbids padding zeros.
    // If the first char is zero, it must be the only char.
    if c < '0' as int || c > '9' as int {
        fail ~"Not a tnetstring: invalid or missing length prefix";
    } else if c == '0' as int {
        c = reader.read_byte();
    } else {
        loop {
            len = (10u * len) + ((c as uint) - ('0' as uint));

            if reader.eof() {
                fail ~"Not a tnetstring: invalid or missing length prefix";
            }
            c = reader.read_byte();

            if c < '0' as int || c > '9' as int {
                break;
            }
        }
    }

    // Validate end-of-length-prefix marker.
    if c != ':' as int {
        fail ~"Not a tnetstring: missing length prefix";
    }

    // Read the data plus terminating type tag.
    let payload = reader.read_bytes(len);

    if payload.len() != len {
        fail ~"Not a tnetstring: invalid length prefix";
    }

    if reader.eof() {
        fail ~"Not a tnetstring: missing type tag";
    }

    match reader.read_byte() as char {
      '#' => {
        let s = unsafe { str::unsafe::from_bytes(payload) };
        int::from_str(s).map(|v| int(v))
      }
      '}' => Some(map(parse_map(payload))),
      ']' => Some(vec(parse_vec(payload))),
      '!' => {
        let s = unsafe { str::unsafe::from_bytes(payload) };
        bool::from_str(s).map(|v| bool(v))
      }
      '^' => {
        let s = unsafe { str::unsafe::from_bytes(payload) };
        float::from_str(s).map(|v| float(v))
      }
      '~' => {
        assert payload.len() == 0u;
        Some(null)
      }
      ',' => Some(str(payload)),
      c => {
        let s = str::from_char(c);
        fail #fmt("Invalid payload type: %?", s)
      }
    }
}

fn parse_vec(data: ~[u8]) -> ~[tnetstring] {
    if data.len() == 0u { return ~[]; }

    do io::with_bytes_reader(data) |reader| {
        let mut result = ~[];

        match move from_reader(reader) {
            Some(move value) => vec::push(result, value),
            None => fail ~"invalid value"
        }

        while !reader.eof() {
            match move from_reader(reader) {
                Some(move value) => vec::push(result, value),
                None => fail ~"invalid tnetstring"
            }
        }

        result
    }
}

fn parse_pair(reader: io::Reader) -> (~[u8], tnetstring) {
    match move from_reader(reader) {
        Some(str(move key)) => {
            match move from_reader(reader) {
                Some(move value) => (key, value),
                None => fail ~"invalid tnetstring",
            }
        }
        Some(_) => fail ~"Keys can only be strings.",
        None => fail ~"Invalid tnetstring",
    }
}

fn parse_map(data: ~[u8]) -> ~send_map::linear::LinearMap<~[u8], tnetstring> {
    let result = ~send_map::linear::LinearMap();

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

/// Deserializes a tnetstring value from a byte string.
fn from_bytes(data: ~[u8]) -> (Option<tnetstring>, ~[u8]) {
    do io::with_bytes_reader(data) |reader| {
        let tnetstring = from_reader(reader);
        (tnetstring, reader.read_whole_stream())
    }
}

/// Deserializes a tnetstring value from a string.
fn from_str(data: &str) -> (Option<tnetstring>, ~str) {
    do io::with_str_reader(data) |rdr| {
        let tnetstring = from_reader(rdr);
        let bytes = rdr.read_whole_stream();
        (tnetstring, str::from_bytes(bytes))
    }

}

/// Test the equality between two tnetstring values
fn eq(t0: &tnetstring, t1: &tnetstring) -> bool {
    // FIXME: https://github.com/mozilla/rust/issues/2855
    match (copy *t0, copy *t1) {
        (str(s0), str(s1)) => s0 == s1,
        (int(i0), int(i1)) => i0 == i1,
        (float(f0), float(f1)) => f0 == f1,
        (bool(b0), bool(b1)) => b0 == b1,
        (null, null) => true,
        (map(d0), map(d1)) => {
            if d0.len() == d1.len() {
                for d0.each_ref |k0, v0| {
                    let result = do d1.with_find_ref(k0) |v1| {
                        match v1 {
                            Some(v1) => eq(v0, v1),
                            None => false,
                        }
                    };
                    if !result { return false; }
                }
                true
            } else {
                false
            }
        }
        (vec(v0), vec(v1)) => vec::all2(v0, v1, |x0, x1| eq(&x0, &x1)),
        _ => false
    }
}

#[cfg(test)]
mod tests {
    // Tests inspired by https://github.com/rfk/tnetstring.

    fn test(string: ~str, expected: tnetstring) {
        let (actual, rest) = from_str(string);
        assert actual.is_some();
        assert rest == ~"";

        let actual = option::unwrap(actual);
        assert eq(&actual, &expected);
        assert to_str(expected) == string;
    }

    #[test]
    fn test_format() {
        test(~"11:hello world,", str(str::to_bytes("hello world")));
        test(~"0:}", map(~send_map::linear::LinearMap()));
        test(~"0:]", vec(~[]));

        let d = ~send_map::linear::LinearMap();
        d.insert(str::to_bytes("hello"),
                vec(~[
                    int(12345678901),
                    str(str::to_bytes("this")),
                    bool(true),
                    null,
                    str(str::to_bytes("\x00\x00\x00\x00"))]));

        test(~"51:5:hello,39:11:12345678901#4:this,4:true!0:~4:\x00\x00\x00\
               \x00,]}", map(d));

        test(~"5:12345#", int(12345));
        test(~"12:this is cool,", str(str::to_bytes("this is cool")));
        test(~"0:,", str(str::to_bytes("")));
        test(~"0:~", null);
        test(~"4:true!", bool(true));
        test(~"5:false!", bool(false));
        test(~"10:\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00,",
            str(str::to_bytes("\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00")));
        test(~"24:5:12345#5:67890#5:xxxxx,]",
            vec(~[
                int(12345),
                int(67890),
                str(str::to_bytes("xxxxx"))]));
        test(~"18:3:0.1^3:0.2^3:0.4^]",
           vec(~[float(0.1), float(0.2), float(0.4)]));
        test(~"243:238:233:228:223:218:213:208:203:198:193:188:183:178:173:\
               168:163:158:153:148:143:138:133:128:123:118:113:108:103:99:95:\
               91:87:83:79:75:71:67:63:59:55:51:47:43:39:35:31:27:23:19:15:\
               11:hello-there,]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]\
               ]]]]",
            vec(
                ~[vec(~[vec(~[vec(~[vec(~[vec(~[vec(~[vec(~[vec(
                ~[vec(~[vec(~[vec(~[vec(~[vec(~[vec(~[vec(~[vec(
                ~[vec(~[vec(~[vec(~[vec(~[vec(~[vec(~[vec(~[vec(
                ~[vec(~[vec(~[vec(~[vec(~[vec(~[vec(~[vec(~[vec(
                ~[vec(~[vec(~[vec(~[vec(~[vec(~[vec(~[vec(~[vec(
                ~[vec(~[vec(~[vec(~[vec(~[vec(~[vec(~[vec(~[vec(
                ~[vec(~[vec(~[
                    str(str::to_bytes("hello-there"))
                ])])])])])])])])])])])])])])])])])])])])])])])])])])])])
                ])])])])])])])])])])])])])])])])])])])])])])]));
    }

    #[test]
    fn test_random() {
        fn randint(rng: rand::Rng, a: u32, b: u32) -> u32 {
            (rng.next() % (b - a + 1u32)) + a
        }

        fn get_random_object(rng: rand::Rng, depth: u32) -> tnetstring {
            if randint(rng, depth, 10u32) <= 4u32 {
                if randint(rng, 0u32, 1u32) == 0u32 {
                    let n = randint(rng, 0u32, 10u32);
                    vec(vec::from_fn(n as uint, |_i|
                        get_random_object(rng, depth + 1u32)
                    ))
                } else {
                    let d = ~send_map::linear::LinearMap();

                    let mut i = randint(rng, 0u32, 10u32);
                    while i != 0u32 {
                        let s = rng.gen_bytes(randint(rng, 0u32, 100u32) as uint);
                        d.insert(s, get_random_object(rng, depth + 1u32));
                        i -= 1u32;
                    }
                    map(d)
                }
            } else {
                match randint(rng, 0u32, 5u32) {
                  0u32 => null,
                  1u32 => bool(true),
                  2u32 => bool(false),
                  3u32 => {
                    if randint(rng, 0u32, 1u32) == 0u32 {
                        int(rng.next() as int)
                    } else {
                        int(-rng.next() as int)
                    }
                  }
                  4u32 => {
                    let mut f = rng.gen_float();

                    // Generate a float that can be exactly converted to
                    // and from a string.
                    loop {
                        match float::from_str(float::to_str(f, 6u)) {
                          Some(f1) => {
                            if f == f1 { break; }
                            f = f1;
                          }
                          None => fail ~"invalid float"
                        }
                    }

                    if randint(rng, 0u32, 1u32) == 0u32 {
                        float(f)
                    } else {
                        float(-f)
                    }
                  }
                  5u32 => {
                    str(rng.gen_bytes(randint(rng, 0u32, 100u32) as uint))
                  }
                  _ => fail
                }
            }
        }

        let rng = rand::Rng();

        let mut i = 500u;
        while i != 0u {
            let v0 = get_random_object(rng, 0u32);
            
            match from_bytes(to_bytes(v0)) {
                (Some(v1), rest) if rest == ~[] => assert eq(&v0, &v1),
                _ => fail ~"invalid tnetstring"
            }
            i -= 1u;
        }
    }
}
