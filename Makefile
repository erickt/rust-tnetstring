RUSTPKG ?= rustpkg
RUST_FLAGS ?= -Z debug-info -O

all:
	$(RUSTPKG) $(RUST_FLAGS) install tnetstring

test:
	$(RUSTPKG) test tnetstring

clean:
	rm -rf bin build lib
