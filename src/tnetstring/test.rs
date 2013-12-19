extern mod tnetstring;

use std::f64;
use std::hashmap::HashMap;
use std::rand;
use std::rand::Rng;
use std::vec;

use tnetstring::TNetString;
use tnetstring::{Str, Int, Float, Bool, Null, Map, Vec};
use tnetstring::{from_bytes, to_bytes};
use tnetstring::from_str;

// Tests inspired by https://github.com/rfk/TNetString.

fn test(s: &str, expected: &TNetString) {
    let (actual, mut rest) = from_str(s);
    assert!(actual.is_some());
    assert!(rest.eof());

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
        if rng.gen_range(depth, 10u32) <= 4u32 {
            if rng.gen_range(0u32, 1u32) == 0u32 {
                let n = rng.gen_range(0u32, 10u32);
                Vec(vec::from_fn(n as uint, |_i|
                    get_random_object(rng, depth + 1u32)
                ))
            } else {
                let mut d = ~HashMap::new();

                let mut i = rng.gen_range(0u32, 10u32);
                while i != 0u32 {
                    let n = rng.gen_range(0u32, 100u32) as uint;
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
            match rng.gen_range(0u32, 5u32) {
              0u32 => Null,
              1u32 => Bool(true),
              2u32 => Bool(false),
              3u32 => {
                if rng.gen_range(0u32, 1u32) == 0u32 {
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

                if rng.gen_range(0u32, 1u32) == 0u32 {
                    Float(f)
                } else {
                    Float(-f)
                }
              }
              5u32 => {
                let n = rng.gen_range(0u32, 100u32) as uint;
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
        let bytes = to_bytes(&v0);

        match from_bytes(bytes) {
            (Some(ref v1), mut rest) if rest.eof() => {
                assert!(v0 == *v1)
            },
            _ => fail!("invalid TNetString")
        }
        i -= 1u;
    }
}
