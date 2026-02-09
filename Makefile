# Default to release build, use DEBUG=1 to build debug
PROFILE ?= release
CARGO_FLAGS = --release

ifeq ($(DEBUG),1)
	PROFILE = debug
	CARGO_FLAGS =
endif

all: build

build:
	cargo build $(CARGO_FLAGS)

clean:
	cargo clean

test:
	cargo test

perftest:
	./tests/bench.sh

test_all: test perftest

.PHONY: all build clean install test perftest test_all
