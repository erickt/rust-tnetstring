// Rust TNetStrings serialization library.

use std;
import option::{some, none};
import std::io;
import std::map;
import std::rand;

import io::writer_util;
import io::reader_util;

export t;
export to_writer;
export to_bytes;
export to_str;
export from_reader;
export from_bytes;
export from_str;
export string;
export integer;
export floating;
export boolean;
export null;
export map;
export list;

/*
Tag: t

Represents a tnetstring value.
*/
enum t {
    /* Variant: string */
    string([u8]),
    /* Variant: integer */
    integer(int),
    /* Variant: floating */
    floating(float),
    /* Variant: boolean */
    boolean(bool),
    /* Variant: null */
    null,
    /* Variant: map */
    map(map::hashmap<[u8], t>),
    /* Variant: list */
    list(@[t]),
}

/*
Function: to_writer

Serializes a tnetstring value into a io::writer.
*/
fn to_writer(writer: io::writer, t: t) {
    alt t {
        string(s) {
            writer.write_str(#fmt("%u:", vec::len(s)));
            writer.write(s);
            writer.write_char(',');
        }
        integer(i) {
            let s = int::str(i);
            writer.write_str(#fmt("%u:%s#", str::byte_len(s), s));
        }
        floating(f) {
            let s = float::to_str(f, 6u);
            writer.write_str(#fmt("%u:%s^", str::byte_len(s), s));
        }
        boolean(b) {
            let s = bool::to_str(b);
            writer.write_str(#fmt("%u:%s!", str::byte_len(s), s));
        }
        map(m) {
            let buf = io::mk_mem_buffer();
            let wr = io::mem_buffer_writer(buf);
            m.items({ |key, value|
                to_writer(wr, string(key));
                to_writer(wr, value);
            });
            let payload = io::mem_buffer_buf(buf);
            writer.write_str(#fmt("%u:", vec::len(payload)));
            writer.write(payload);
            writer.write_char('}');
        }
        list(@l) {
            let buf = io::mk_mem_buffer();
            let wr = io::mem_buffer_writer(buf);
            vec::iter(l, {|e| to_writer(wr, e) });
            let payload = io::mem_buffer_buf(buf);
            writer.write_str(#fmt("%u:", vec::len(payload)));
            writer.write(payload);
            writer.write_char(']');
        }
        null {
            writer.write_str("0:~");
        }
    }
}

/*
Function: to_bytes

Serializes a tnetstring value into a byte string.
*/
fn to_bytes(t: t) -> [u8] {
    let buf = io::mk_mem_buffer();
    let wr = io::mem_buffer_writer(buf);
    to_writer(wr, t);
    io::mem_buffer_buf(buf)
}

/*
Function: to_str

Serializes a tnetstring value into a string.
*/
fn to_str(t: t) -> str {
    let buf = io::mk_mem_buffer();
    let wr = io::mem_buffer_writer(buf);
    to_writer(wr, t);
    io::mem_buffer_str(buf)
}

/*
Function: from_reader

Deserializes a tnetstring value from an io::reader.
*/
fn from_reader(reader: io::reader) -> option::t<t> {
    assert !reader.eof();

    let c = reader.read_byte();
    let len = 0u;

    // Note that netstring spec explicitly forbids padding zeros.
    // If the first char is zero, it must be the only char.
    if c < '0' as int || c > '9' as int {
        fail "Not a tnetstring: invalid or missing length prefix";
    } else if c == '0' as int {
        c = reader.read_byte();
    } else {
        do {
            len = (10u * len) + ((c as uint) - ('0' as uint));

            if reader.eof() {
                fail "Not a tnetstring: invalid or missing length prefix";
            }
            c = reader.read_byte();
        } while c >= '0' as int && c <= '9' as int;
    }

    // Validate end-of-length-prefix marker.
    if c != ':' as int {
        fail "Not a tnetstring: missing length prefix";
    }

    // Read the data plus terminating type tag.
    let payload = reader.read_bytes(len);

    if vec::len(payload) != len {
        fail "Not a tnetstring: invalid length prefix";
    }

    if reader.eof() {
        fail "Not a tnetstring: missing type tag";
    }

    alt reader.read_byte() as char {
        '#' {
            let v = int::from_str(str::unsafe_from_bytes(payload));
            some(integer(v))
        }
        '}' { some(map(parse_map(payload))) }
        ']' { some(list(parse_list(payload))) }
        '!' {
            let v = bool::from_str(str::unsafe_from_bytes(payload));
            some(boolean(v))
        }
        '^' {
            let v = float::from_str(str::unsafe_from_bytes(payload));
            some(floating(v))
        }
        '~' {
            assert vec::len(payload) == 0u;
            some(null)
        }
        ',' { some(string(payload)) }
        c {
            let s = str::from_char(c);
            fail #fmt("Invalid payload type: %s", str::escape(s))
        }
    }
}

fn parse_list(data: [u8]) -> @[t] {
    if vec::len(data) == 0u { ret @[]; }

    let reader = io::bytes_reader(data);

    let value = from_reader(reader);
    assert option::is_some(value);
    let result = [option::get(value)];

    while !reader.eof() {
        let value = from_reader(reader);
        assert option::is_some(value);
        vec::push(result, option::get(value));
    }

    ret @result;
}

fn parse_pair(reader: io::reader) -> ([u8], t) {
    let key = from_reader(reader);
    assert option::is_some(key);
    assert !reader.eof();

    let key = alt option::get(key) {
        string(key) { key }
        _ { fail "Keys can only be strings." }
    };

    let value = from_reader(reader);
    assert option::is_some(value);

    ret (key, option::get(value));
}

fn parse_map(data: [u8]) -> map::hashmap<[u8], t> {
    if vec::len(data) == 0u { ret map::new_bytes_hash(); }

    let reader = io::bytes_reader(data);
    let (key, value) = parse_pair(reader);

    let result = map::new_bytes_hash();
    result.insert(key, value);

    while !reader.eof() {
        let (key, value) = parse_pair(reader);
        result.insert(key, value);
    }

    ret result;
}

/*
Function: from_bytes

Deserializes a tnetstring value from a byte string.
*/
fn from_bytes(data: [u8]) -> (option::t<t>, [u8]) {
    let reader = io::bytes_reader(data);
    let tnetstring = from_reader(reader);
    (tnetstring, reader.read_whole_stream())
}

/*
Function: from_str

Deserializes a tnetstring value from a string.
*/
fn from_str(data: str) -> (option::t<t>, str) {
    let reader = io::string_reader(data);
    let tnetstring = from_reader(reader);
    (tnetstring, str::unsafe_from_bytes(reader.read_whole_stream()))
}

fn eq(t0: t, t1: t) -> bool {
    alt (t0, t1) {
        (string(s0), string(s1)) { s0 == s1 }
        (integer(i0), integer(i1)) { i0 == i1 }
        (floating(f0), floating(f1)) { f0 == f1 }
        (boolean(b0), boolean(b1)) { b0 == b1 }
        (null, null) { true }
        (map(d0), map(d1)) {
            if d0.size() == d1.size() {
                let equal = true;
                d0.items() { |k, v|
                    if !d1.contains_key(k) || !eq(d1.get(k), v) {
                        equal = false;
                    }
                };
                equal
            } else {
                false
            }
        }
        (list(l0), list(l1)) {
            vec::all2(*l0, *l1, eq)
        }
        _ { false }
    }
}

#[cfg(test)]
mod tests {
    // Tests inspired by https://github.com/rfk/tnetstring.

    fn test(string: str, expected: t) {
        let (actual, rest) = from_str(string);
        assert option::is_some(actual);
        assert rest == "";

        let actual = option::get(actual);
        assert eq(actual, expected);
        assert to_str(expected) == string;
    }

    #[test]
    fn test_format() {
        test("11:hello world,", string(str::bytes("hello world")));
        test("0:}", map(map::new_bytes_hash()));
        test("0:]", list(@[]));

        let d = map::new_bytes_hash();
        d.insert(str::bytes("hello"),
                list(@[
                    integer(12345678901),
                    string(str::bytes("this")),
                    boolean(true),
                    null,
                    string(str::bytes("\x00\x00\x00\x00"))]));

        test("51:5:hello,39:11:12345678901#4:this,4:true!0:~4:\x00\x00\x00" +
            "\x00,]}", map(d));

        test("5:12345#", integer(12345));
        test("12:this is cool,", string(str::bytes("this is cool")));
        test("0:,", string(str::bytes("")));
        test("0:~", null);
        test("4:true!", boolean(true));
        test("5:false!", boolean(false));
        test("10:\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00,",
            string(str::bytes("\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00")));
        test("24:5:12345#5:67890#5:xxxxx,]",
            list(
                @[integer(12345),
                integer(67890),
                string(str::bytes("xxxxx"))]));
        test("18:3:0.1^3:0.2^3:0.4^]",
           list(@[floating(0.1), floating(0.2), floating(0.4)]));
        test("243:238:233:228:223:218:213:208:203:198:193:188:183:178:173:" +
            "168:163:158:153:148:143:138:133:128:123:118:113:108:103:99:95:" +
            "91:87:83:79:75:71:67:63:59:55:51:47:43:39:35:31:27:23:19:15:" +
            "11:hello-there,]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]" +
            "]]]]",
            list(
                @[list(@[list(@[list(@[list(@[list(@[list(@[list(@[list(
                @[list(@[list(@[list(@[list(@[list(@[list(@[list(@[list(
                @[list(@[list(@[list(@[list(@[list(@[list(@[list(@[list(
                @[list(@[list(@[list(@[list(@[list(@[list(@[list(@[list(
                @[list(@[list(@[list(@[list(@[list(@[list(@[list(@[list(
                @[list(@[list(@[list(@[list(@[list(@[list(@[list(@[list(
                @[list(@[list(@[
                    string(str::bytes("hello-there"))
                ])])])])])])])])])])])])])])])])])])])])])])])])])])])])
                ])])])])])])])])])])])])])])])])])])])])])])]));
    }

    #[test]
    fn test_random() {
        fn randint(rng: rand::rng, a: u32, b: u32) -> u32 {
            (rng.next() % (b - a + 1u32)) + a
        }

        fn get_random_object(rng: rand::rng, depth: u32) -> t {
            if randint(rng, depth, 10u32) <= 4u32 {
                if randint(rng, 0u32, 1u32) == 0u32 {
                    let n = randint(rng, 0u32, 10u32);
                    list(@vec::init_fn(
                        { |_i| get_random_object(rng, depth + 1u32) },
                        n as uint))
                } else {
                    let d = map::new_bytes_hash();

                    let i = randint(rng, 0u32, 10u32);
                    while i != 0u32 {
                        let s = rng.gen_bytes(randint(rng, 0u32, 100u32) as uint);
                        d.insert(s, get_random_object(rng, depth + 1u32));
                        i -= 1u32;
                    }
                    map(d)
                }
            } else {
                alt randint(rng, 0u32, 5u32) {
                    0u32 { null }
                    1u32 { boolean(true) }
                    2u32 { boolean(false) }
                    3u32 {
                        if randint(rng, 0u32, 1u32) == 0u32 {
                            integer(rng.next() as int)
                        } else {
                            integer(-rng.next() as int)
                        }
                    }
                    4u32 {
                        let f = rng.next_float();

                        // Generate a float that can be exactly converted to
                        // and from a string.
                        while true {
                            let f1 = float::from_str(float::to_str(f, 6u));
                            if f == f1 { break; }
                            f = f1;
                        }

                        if randint(rng, 0u32, 1u32) == 0u32 {
                            floating(f)
                        } else {
                            floating(-f)
                        }
                    }
                    5u32 {
                        string(rng.gen_bytes(randint(rng, 0u32, 100u32) as uint))
                    }
                    _ { fail }
                }
            }
        }

        let rng = rand::mk_rng();

        let i = 500u;
        while i != 0u {
            let v0 = get_random_object(rng, 0u32);
            
            alt from_str(to_str(v0)) {
                (some(v1), "") { assert eq(v0, v1); }
                _ { fail; }
            }
            i -= 1u;
        }
    }
}
