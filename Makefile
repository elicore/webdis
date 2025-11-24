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

# Docker helpers
DOCKER_IMAGE ?= elicore/webdis:latest

docker-build:
	@echo "Building $(DOCKER_IMAGE)..."
	docker build -t $(DOCKER_IMAGE) .

docker-build-dev:
	@echo "Building local dev image 'webdis:dev'..."
	docker build -t webdis:dev .

docker-push:
	@echo "Pushing $(DOCKER_IMAGE)..."
	docker push $(DOCKER_IMAGE)

docker-sign:
	@echo "Signing $(DOCKER_IMAGE) with cosign..."
	@if [ -z "$(COSIGN_KEY_FILE)" ]; then \
		echo "COSIGN_KEY_FILE is not set; skipping signature"; \
		exit 0; \
	fi; \
	cosign sign --key $(COSIGN_KEY_FILE) $(DOCKER_IMAGE)

docker-publish: docker-build
	@echo "Publishing $(DOCKER_IMAGE) and signing if key is provided..."
	docker push $(DOCKER_IMAGE)
	@if [ -n "$(COSIGN_KEY_FILE)" ]; then \
		cosign sign --key $(COSIGN_KEY_FILE) $(DOCKER_IMAGE); \
	fi

compose-up-dev:
	@echo "Starting development compose (docker-compose.dev.yml)"
	docker compose -f docker-compose.dev.yml up --build

compose-down-dev:
	@echo "Stopping development compose (docker-compose.dev.yml) and removing volumes"
	docker compose -f docker-compose.dev.yml down -v

.PHONY: all build clean install test perftest test_all
