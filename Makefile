RUSTC ?= rustc
RUST_FLAGS ?= -O

tnetstring_files = \
									 src/tnetstring/lib.rs

all: tnetstring

build:
	mkdir -p build

tnetstring: build $(tnetstring_files)
	$(RUSTC) $(RUST_FLAGS) src/tnetstring/lib.rs --out-dir=build/

build/tests: build $(tnetstring_files)
	$(RUSTC) $(RUST_FLAGS) --test src/tnetstring/lib.rs -o build/tests

check: build/tests
	./build/tests --test

clean:
	rm -rf bin build lib
