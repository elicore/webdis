OUT=webdis
PREFIX ?= /usr/local
CONFDIR ?= $(DESTDIR)/etc
INSTALL_DIRS = $(DESTDIR)$(PREFIX)/bin \
	       $(CONFDIR)

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
	cp target/$(PROFILE)/$(OUT) .

clean:
	cargo clean
	rm -f $(OUT)

install: build $(INSTALL_DIRS)
	cp $(OUT) $(DESTDIR)$(PREFIX)/bin
	cp webdis.prod.json $(CONFDIR)

$(INSTALL_DIRS):
	mkdir -p $@

test:
	cargo test

perftest:
	./tests/bench.sh

test_all: test perftest

.PHONY: all build clean install test perftest test_all
