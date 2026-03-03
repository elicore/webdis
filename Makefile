# Default to release build, use DEBUG=1 to build debug
PROFILE ?= release
CARGO_FLAGS = --release
ACT ?= act
ACT_IMAGE ?= ghcr.io/catthehacker/ubuntu:act-24.04
ACT_CONTAINER_ARCH ?= linux/amd64

ifeq ($(DEBUG),1)
	PROFILE = debug
	CARGO_FLAGS =
endif

all: build

build:
	cargo build --workspace $(CARGO_FLAGS)

build_hiredis_compat:
	./scripts/build-hiredis-compat.sh

test_hiredis_compat_fixture:
	./crates/redis-web-hiredis-compat/tests/compile-fixture.sh

bench_hiredis_compat:
	./crates/redis-web/tests/bench-hiredis-compat.sh

clean:
	cargo clean

test:
	cargo test -p redis-web --lib
	cargo test -p redis-web --test config_test --test handler_test --test logging_fsync_test --test functional_interface_mapping_test --test functional_http_contract_test --test functional_ws_contract_test

test_unit:
	cargo test --workspace --lib

test_functional:
	cargo test -p redis-web --test config_test --test handler_test --test logging_fsync_test --test functional_interface_mapping_test --test functional_http_contract_test --test functional_ws_contract_test

test_integration:
	cargo test -p redis-web --test integration_process_boot_test --test integration_redis_http_test --test integration_redis_pubsub_test --test integration_redis_socket_test --test websocket_raw_test --test integration_hiredis_compat_test

perftest:
	./crates/redis-web/tests/bench.sh

test_all: test perftest

ci_local_linux:
	$(ACT) pull_request -W .github/workflows/build.yml -j ci-linux \
		--matrix runner:ubuntu-24.04 \
		--matrix os_name:ubuntu-24.04 \
		--matrix arch:x86_64 \
		-P ubuntu-24.04=$(ACT_IMAGE) \
		--container-architecture $(ACT_CONTAINER_ARCH)

ci_local_linux_arm:
	$(ACT) pull_request -W .github/workflows/build.yml -j ci-linux \
		--matrix runner:ubuntu-24.04-arm \
		--matrix os_name:ubuntu-24.04-arm \
		--matrix arch:aarch64 \
		-P ubuntu-24.04-arm=$(ACT_IMAGE) \
		--container-architecture $(ACT_CONTAINER_ARCH)

ci_local: ci_local_linux ci_local_linux_arm

.PHONY: all build build_hiredis_compat test_hiredis_compat_fixture bench_hiredis_compat clean install test perftest test_all ci_local ci_local_linux ci_local_linux_arm
