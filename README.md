TNetStrings: Tagged Netstrings

This module implements bindings for the [tnetstring](http://tnetstrings.org)
serialization format.

## API

    let t = tnetstrings::str("hello world");
    let s = tnetstrings::to_str(t) // returns "11:hello world,"

    let (t, extra) = tnetstrings::from_str(s);
    alt option::get(t) {
      tnetstrings::str(s) { ... }
      ...
    }

See the `tests` module in `tnetstrings.rs` for more examples.
