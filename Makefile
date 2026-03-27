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
	cargo build $(CARGO_FLAGS)

build_hiredis_compat:
	./scripts/build-hiredis-compat.sh

test_hiredis_compat_fixture:
	./crates/redis-web-hiredis-compat/tests/compile-fixture.sh

# Optional compat / perf / benchmark surfaces stay explicit.
bench_hiredis_compat:
	./crates/redis-web/tests/bench-hiredis-compat.sh

bench_config_compare:
	cargo run -p redis-web-bench -- compare --spec "$(SPEC)"

compat_redispy_bootstrap:
	./subprojects/redispy-hiredis-compat/scripts/bootstrap.sh

compat_redispy_build_hiredis:
	./subprojects/redispy-hiredis-compat/scripts/build-hiredis-wheel.sh

compat_redispy_test:
	./subprojects/redispy-hiredis-compat/scripts/run-redispy-tests.sh

compat_redispy_audit:
	./subprojects/redispy-hiredis-compat/scripts/audit-hiredis-symbols.sh

compat_redispy_regression:
	./subprojects/redispy-hiredis-compat/scripts/test-setup-test-env.sh

compat_runtime_matrix:
	./crates/redis-web-hiredis-compat/tests/compile-abi-layout.sh
	./crates/redis-web-hiredis-compat/tests/compile-runtime-ssl-symbols.sh
	./subprojects/redispy-hiredis-compat/scripts/audit-no-unsupported-sync.sh

compat_async_matrix:
	./subprojects/redispy-hiredis-compat/scripts/run-redispy-runtime-matrix.sh

compat_no_unsupported_sync_audit:
	./subprojects/redispy-hiredis-compat/scripts/audit-no-unsupported-sync.sh

compat_ssl_audit:
	./subprojects/redispy-hiredis-compat/scripts/audit-hiredis-symbols.sh
	./crates/redis-web-hiredis-compat/tests/compile-runtime-ssl-symbols.sh

clean:
	cargo clean

# Core server path: fast unit + contract checks that do not require the heavier
# compat, gRPC, or performance harnesses.
test:
	cargo test -p redis-web --lib
	cargo test -p redis-web --test config_test --test handler_test --test functional_interface_mapping_test --test functional_http_contract_test --test functional_ws_contract_test

test_unit:
	cargo test --workspace --lib

test_functional:
	cargo test -p redis-web --test config_test --test handler_test --test functional_interface_mapping_test --test functional_http_contract_test --test functional_ws_contract_test

test_integration:
	$(MAKE) test_integration_core

test_integration_core:
	cargo test -p redis-web --test integration_process_boot_test --test integration_redis_http_test --test integration_redis_pubsub_test --test integration_redis_socket_test --test websocket_raw_test

test_grpc:
	cargo test -p redis-web --test functional_grpc_contract_test --test integration_redis_grpc_test

test_compat:
	cargo test -p redis-web --test integration_hiredis_compat_test

perftest:
	./crates/redis-web/tests/bench.sh

test_all: test test_integration_core test_grpc test_compat perftest

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

.PHONY: all build build_hiredis_compat test_hiredis_compat_fixture bench_hiredis_compat bench_config_compare compat_redispy_bootstrap compat_redispy_build_hiredis compat_redispy_test compat_redispy_audit compat_redispy_regression compat_runtime_matrix compat_async_matrix compat_no_unsupported_sync_audit compat_ssl_audit clean install test test_integration test_integration_core test_grpc test_compat perftest test_all ci_local ci_local_linux ci_local_linux_arm
