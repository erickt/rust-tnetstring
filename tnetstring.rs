// Rust TNetStrings serialization library.

use std;
import option::{some, none};
import std::map;
import std::rand;

export t;
export to_str;
export from_str;
export str;
export int;
export float;
export bool;
export null;
export map;
export vec;

/*
Tag: t

Represents a tnetstring value.
*/
tag t {
    /* Variant: str */
    str(str);
    /* Variant: int */
    int(int);
    /* Variant: float */
    float(float);
    /* Variant: bool */
    bool(bool);
    /* Variant: null */
    null;
    /* Variant: map */
    map(map::hashmap<str, t>);
    /* Variant: vec */
    vec(@[t]);
}

/*
Function: to_str

Serializes a tnetstring value into a string.
*/
fn to_str(t: t) -> str {
    alt t {
        str(s) { #fmt("%u:%s,", str::byte_len(s), s) }
        int(i) {
            let s = int::str(i);
            #fmt("%u:%s#", str::byte_len(s), s)
        }
        float(f) {
            let s = #fmt("%f", f);
            #fmt("%u:%s^", str::byte_len(s), s)
        }
        bool(b) {
            let s = bool::to_str(b);
            #fmt("%u:%s!", str::byte_len(s), s)
        }
        map(m) {
            let result = [];
            vec::reserve(result, m.size() * 2u);
            m.items({ |k, v|
                vec::push(result, to_str(str(k)));
                vec::push(result, to_str(v));
            });
            let payload = str::concat(result);
            #fmt("%u:%s}", str::byte_len(payload), payload)
        }
        vec(@l) {
            let result = vec::map(l, { |e| to_str(e) });
            let payload = str::concat(result);
            #fmt("%u:%s]", str::byte_len(payload), payload)
        }
        null { "0:~" }
    }
}

/*
Function: from_str

Deserializes a tnetstring value from a string.
*/
fn from_str(data: str) -> (option::t<t>, str) {
    let (payload, payload_type, remain) = from_str_payload(data);

    let value = alt payload_type {
        '#' { some(int(int::from_str(payload))) }
        '}' { some(map(from_str_map(payload))) }
        ']' { some(vec(from_str_vec(payload))) }
        '!' { some(bool(bool::from_str(payload))) }
        '^' { some(float(float::from_str(payload))) }
        '~' {
            assert str::byte_len(payload) == 0u;
            some(null)
        }
        ',' { some(str(payload)) }
        _ {
            let s = str::from_char(payload_type);
            fail #fmt("Invalid payload type: %s", str::escape(s))
        }
    };

    ret (value, remain);
}

fn from_str_payload(data: str) -> (str, char, str) {
    assert data != "";

    let parts = str::splitn(data, ':' as u8, 1u);
    let (length, extra) = alt vec::len(parts) {
        0u { ("", "") }
        1u { (parts[0], "") }
        2u { (parts[0], parts[1]) }
        _ { fail }
    };
    let length = uint::from_str(length);

    let payload = str::slice(extra, 0u, length);
    let extra = str::slice(extra, length, str::byte_len(extra));
    assert extra != "";

    let payload_type = str::char_at(extra, 0u);
    let remain = str::slice(extra, 1u, str::byte_len(extra));

    assert str::byte_len(payload) == length;
    ret (payload, payload_type, remain);
}

fn from_str_vec(data: str) -> @[t] {
    if str::byte_len(data) == 0u { ret @[]; }

    let (value, extra) = from_str(data);
    let result = [option::get(value)];

    while extra != "" {
        let (value, e) = from_str(extra);
        extra = e;
        vec::push(result, option::get(value));
    }

    ret @result;
}

fn from_str_pair(data: str) -> (t, t, str) {
    let (key, extra) = from_str(data);
    assert option::is_some(key);
    assert extra != "";

    let (value, extra) = from_str(extra);
    assert option::is_some(value);

    ret (option::get(key), option::get(value), extra);
}

fn from_str_map(data: str) -> map::hashmap<str, t> {
    if str::byte_len(data) == 0u { ret map::new_str_hash(); }

    let (key, value, extra) = from_str_pair(data);

    let key = alt key {
        str(key) { key }
        _ { fail "Keys can only be strings." }
    };

    let result = map::new_str_hash();
    result.insert(key, value);

    while extra != "" {
        let (key, value, e) = from_str_pair(extra);
        extra = e;

        let key = alt key {
            str(key) { key }
            _ { fail "Keys can only be strings." }
        };

        result.insert(key, value);
    }

    ret result;
}

fn eq(t0: t, t1: t) -> bool {
    alt (t0, t1) {
        (str(s0), str(s1)) { s0 == s1 }
        (int(i0), int(i1)) { i0 == i1 }
        (float(f0), float(f1)) { f0 == f1 }
        (bool(b0), bool(b1)) { b0 == b1 }
        (null., null.) { true }
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
        (vec(l0), vec(l1)) {
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
        test("11:hello world,", str("hello world"));
        test("0:}", map(map::new_str_hash()));
        test("0:]", vec(@[]));

        let d = map::new_str_hash();
        d.insert("hello",
                vec(@[
                    int(12345678901),
                    str("this"),
                    bool(true),
                    null,
                    str("\x00\x00\x00\x00")]));

        test("51:5:hello,39:11:12345678901#4:this,4:true!0:~4:\x00\x00\x00" +
            "\x00,]}", map(d));

        test("5:12345#", int(12345));
        test("12:this is cool,", str("this is cool"));
        test("0:,", str(""));
        test("0:~", null);
        test("4:true!", bool(true));
        test("5:false!", bool(false));
        test("10:\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00,",
            str("\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00"));
        test("24:5:12345#5:67890#5:xxxxx,]",
            vec(@[int(12345), int(67890), str("xxxxx")]));
        test("18:3:0.1^3:0.2^3:0.4^]",
           vec(@[float(0.1), float(0.2), float(0.4)]));
        test("243:238:233:228:223:218:213:208:203:198:193:188:183:178:173:" +
            "168:163:158:153:148:143:138:133:128:123:118:113:108:103:99:95:" +
            "91:87:83:79:75:71:67:63:59:55:51:47:43:39:35:31:27:23:19:15:" +
            "11:hello-there,]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]" +
            "]]]]",
            vec(
                @[vec(@[vec(@[vec(@[vec(@[vec(@[vec(@[vec(@[vec(
                @[vec(@[vec(@[vec(@[vec(@[vec(@[vec(@[vec(@[vec(
                @[vec(@[vec(@[vec(@[vec(@[vec(@[vec(@[vec(@[vec(
                @[vec(@[vec(@[vec(@[vec(@[vec(@[vec(@[vec(@[vec(
                @[vec(@[vec(@[vec(@[vec(@[vec(@[vec(@[vec(@[vec(
                @[vec(@[vec(@[vec(@[vec(@[vec(@[vec(@[vec(@[vec(
                @[vec(@[vec(@[
                    str("hello-there")
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
                    vec(@vec::init_fn(
                        { |_i| get_random_object(rng, depth + 1u32) },
                        n as uint))
                } else {
                    let d = map::new_str_hash();

                    let i = randint(rng, 0u32, 10u32);
                    while i != 0u32 {
                        let s = rng.gen_str(randint(rng, 0u32, 100u32) as uint);
                        d.insert(s, get_random_object(rng, depth + 1u32));
                        i -= 1u32;
                    }
                    map(d)
                }
            } else {
                alt randint(rng, 0u32, 4u32) {
                    0u32 { null }
                    1u32 { bool(true) }
                    2u32 { bool(false) }
                    3u32 {
                        if randint(rng, 0u32, 1u32) == 0u32 {
                            int(rng.next() as int)
                        } else {
                            int(-rng.next() as int)
                        }
                    }
                    /*
                    Disable the float tests for now since it's hard to exactly
                    compare floats.
                    4u32 {
                        if randint(rng, 0u32, 1u32) == 0u32 {
                            float(rng.next_float())
                        } else {
                            float(-rng.next_float())
                        }
                    }
                    */
                    4u32 {
                        str(rng.gen_str(randint(rng, 0u32, 100u32) as uint))
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
                (some(v1), "") {
                    if !eq(v0, v1) {
                        log_err v0;
                        log_err v1;
                    }
                    assert eq(v0, v1);
                }
                _ { fail; }
            }
            i -= 1u;
        }
    }
}
