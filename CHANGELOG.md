# Changelog

All notable changes to this project will be documented in this file.
## [Unreleased]

### Bug Fixes

- *(docs)* Build ([4974b28](https://github.com/elicore/redis-web/commit/4974b28e4c2aa50a3c6d2e1dd8523f61bfe3a429))

- Stabilize hiredis compat session creation in CI ([1a65a60](https://github.com/elicore/redis-web/commit/1a65a60a64864a0c59aa5c2c5ce185226583ca67))


### Documentation

- Rewrite README for Rust implementation and configuration ([bd39e37](https://github.com/elicore/redis-web/commit/bd39e375144ad7cd725f8aa1784f4eb57549b3b9))

- Add embedding guides and interface reference ([647e13b](https://github.com/elicore/redis-web/commit/647e13b57b8a758789409929f527dcb3357f6779))

- Add AGENTS workflow and command guide ([9be089f](https://github.com/elicore/redis-web/commit/9be089f33fb0106adf74c8ab1eeaf834c1cbebf7))

- Migrate to Starlight and add migration/deprecation guidance ([9ff85f4](https://github.com/elicore/redis-web/commit/9ff85f4c8b5403ff68b9b6af0f3a69966765ddc0))

- *(changelog)* Update CHANGELOG.md ([75cf4f8](https://github.com/elicore/redis-web/commit/75cf4f8b1ce099a148df784b486141714fea57bf))

- *(changelog)* Update CHANGELOG.md ([f01c482](https://github.com/elicore/redis-web/commit/f01c4825ef3bd6d93bda321b3b0b6b58209ddc29))

- Consolidate and restructure pages ([99b4246](https://github.com/elicore/redis-web/commit/99b4246f529569fb2938ce8cbf5f2cc3e52f2f5a))

- *(changelog)* Update CHANGELOG.md ([ace33cc](https://github.com/elicore/redis-web/commit/ace33cccf8fd0fdae326023ee7a76038fa1d2154))

- *(changelog)* Update CHANGELOG.md ([51f8519](https://github.com/elicore/redis-web/commit/51f8519be0f8a5d897db8a1c03fbdb14d44dd965))

- Add hiredis compat meta PR plan and task tracker ([3d8cca8](https://github.com/elicore/redis-web/commit/3d8cca81954ab401c6e151c705dd42982bdbcf9e))

- *(changelog)* Update CHANGELOG.md ([6f05165](https://github.com/elicore/redis-web/commit/6f05165ba7e64cf63d0b92d24e14d34f7320e5fc))

- *(changelog)* Update CHANGELOG.md ([382db45](https://github.com/elicore/redis-web/commit/382db4553f86ff7ee5c367dd13024c161c617674))

- *(changelog)* Update CHANGELOG.md ([2c42eb8](https://github.com/elicore/redis-web/commit/2c42eb8de229deb80033a7653f223b09c60796eb))

- *(changelog)* Update CHANGELOG.md ([dfb88bd](https://github.com/elicore/redis-web/commit/dfb88bd5e6c3ebe192a47ef241a99ac3e4a2c2c1))


### Features

- Transition CI to Rust-based build and test, update GitHub Actions matrix with newer Ubuntu, Red Hat, and macOS versions, and upgrade Redis to 8.0. ([4fd54dc](https://github.com/elicore/redis-web/commit/4fd54dc7965476af5104108051cffcd447685052))

- *(config)* Add schema-backed defaults and config generator ([555648f](https://github.com/elicore/redis-web/commit/555648f0d97768e7f77aa4278bd27805a42b569e))

- Add composite actions for Rust setup and build/test workflow ([8f233c4](https://github.com/elicore/redis-web/commit/8f233c4b4163dec0afded972b442c2e71223c348))

- Add workflow for building and deploying Astro site to GitHub Pages ([633b316](https://github.com/elicore/redis-web/commit/633b316c889e882e6810ababc81d1d713994b0c3))

- *(docs)* Replace 'nicolas/webdis' with 'elicore/webdis' across docs and add docker docs ([d60db66](https://github.com/elicore/redis-web/commit/d60db665cea00410c9511153f9a1407ad4428728))

- Implement ETag support for GET requests and add feature list to README  ([bc11cd5](https://github.com/elicore/redis-web/commit/bc11cd567f75bbbdf9488a3b4756468a5a7319d6))

- Implement WebSocket raw RESP endpoint at /.raw   ([f9a9908](https://github.com/elicore/redis-web/commit/f9a990867440c14583807910cfa09069479a4bd3))

- Implement structured JSON output for INFO command  ([dfbac1b](https://github.com/elicore/redis-web/commit/dfbac1b6a9113724b45dae0a533416ea44784eff))

- True raw RESP output for .raw endpoint  ([f84eea6](https://github.com/elicore/redis-web/commit/f84eea60e36550ef4b3d6aad5a25e94f41dda413))

- Upgrade Axum to 0.8 and redis-rs to 0.32, adapting to new routing syntax, WebSocket text types, and RESP3 value variants. ([72830d7](https://github.com/elicore/redis-web/commit/72830d779900ab055f160dcb0d55b0c181b025f8))

- Add JSONP support for HTTP responses  ([f5b1440](https://github.com/elicore/redis-web/commit/f5b1440a7c6f894ba51c73a106decd0d9c1bd76b))

- Expand $VARNAME placeholders in JSON config  ([857f874](https://github.com/elicore/redis-web/commit/857f874db4fa38a10bfc7e99b04070c194c66a09))

- Honor hiredis.keep_alive_sec for Redis TCP keep-alive  ([51306e6](https://github.com/elicore/redis-web/commit/51306e67a2cde62f3a555e3ca5fd9b6315c5cd76))

- URL encoding for `%2f` and `%2e` in command segments  ([4bdbc51](https://github.com/elicore/redis-web/commit/4bdbc519bcecb15ee51b03741f6a9b0f44414902))

- Implement log_fsync durability modes  ([221ecce](https://github.com/elicore/redis-web/commit/221ecce120916d2461b0415bd791336d74def0ab))

- Add chunked HTTP Pub/Sub JSON/JSONP parity   ([bd33979](https://github.com/elicore/redis-web/commit/bd339796e2ac161dea43de7b1544156390333051))

- *(parity)* Database selection via /<db>/ prefix   ([b03b64d](https://github.com/elicore/redis-web/commit/b03b64dd03e117dc998cd1665e1ae584b860dabe))

- Split into redis-web workspace crates with compatibility layer ([96221bb](https://github.com/elicore/redis-web/commit/96221bbb87965393aaaee5f76a84a0de3f6a3ff6))

- Add hiredis-compat session bridge, crate scaffolding, tests, and docs ([ad379ac](https://github.com/elicore/redis-web/commit/ad379aca971e423c158809b01e479a3d785bb0ac))

- *(config)* Add support for creating Config from a values object ([5518c80](https://github.com/elicore/redis-web/commit/5518c802aac0c5a7c7fe1588397dc48db4e44537))


### Miscellaneous Tasks

- Remove CodeQL workflow configuration ([a66739d](https://github.com/elicore/redis-web/commit/a66739d7ba603b350ef59c2e18e2f71f0ad00da4))

- Update workflow triggers to only allow manual dispatch ([100d2d2](https://github.com/elicore/redis-web/commit/100d2d24a8d0329811a72906e4dfc13f6f87ff41))

- *(scripts)* Make helper scripts executable ([780d2ed](https://github.com/elicore/redis-web/commit/780d2ed08faaa8c60c888a0f7fb8c99880619932))

- *(make)* Remove docker helper targets per request ([0f8cce8](https://github.com/elicore/redis-web/commit/0f8cce88dd103b3eb0826c8b834c940795ad77b2))

- Update redis to 1.0.3, cleanup makefile ([7aef3ab](https://github.com/elicore/redis-web/commit/7aef3ab31d06dd46907e734ba2af726e2ec345cd))

- Add json schema to config examples ([d42e626](https://github.com/elicore/redis-web/commit/d42e626ef1e2da12f68a648c437e0f5b05fca866))

- *(deps)* Bump time from 0.3.44 to 0.3.47  ([41653e8](https://github.com/elicore/redis-web/commit/41653e885c8cf392254dbef98a7e2c8ca58f44e6))

- Update runtime ops, compose, and CI workflows for redis-web ([681509a](https://github.com/elicore/redis-web/commit/681509ab66edf2f88166e94ceca2e3807faf1dbb))

- Move prek to local developer config ([28520f5](https://github.com/elicore/redis-web/commit/28520f531c549a38463f245ec5a5d4d35e9a3633))

- Relocate docker assets and config examples to docs ([1b498f9](https://github.com/elicore/redis-web/commit/1b498f9c1a417acf8527cd5ccaa5349dc3b90788))

- *(CI)* Add release-please configuration and manifest files for automated releases ([2b15918](https://github.com/elicore/redis-web/commit/2b1591838501bd2137e249cd0af09d09f22b7a6a))

- *(CI)* Update GitHub Actions workflow to enhance Docker image metadata handling and signing process ([2fe7d43](https://github.com/elicore/redis-web/commit/2fe7d43d75ff06c4ba9f03907c8ecbcfdc3de197))


### Parity

- Redis UNIX socket support  ([b977866](https://github.com/elicore/redis-web/commit/b97786608438b5aca5016ac46f6ca1a0ca4b67cd))

- Custom content types and ?type= MIME overrides  ([e6776a9](https://github.com/elicore/redis-web/commit/e6776a9172b1126b306c16498eaf58307a52f6d4))


### Refactor

- Move tests for parse_info_output to a dedicated test file ([f9e6339](https://github.com/elicore/redis-web/commit/f9e6339d442d7d05ad7e6d3702ad90a4791853ea))


### Refactoring

- Migrate build system to Cargo and improve integration tests with dynamic port allocation. ([c4d975f](https://github.com/elicore/redis-web/commit/c4d975f90f52dfa76e79569f25636e3bbc2b641f))

- Remove legacy Python/C tests and related files, update Rust tests, and add a new README. ([2b4e383](https://github.com/elicore/redis-web/commit/2b4e383b64d0fbc27121af49f8dc84ce60715d9d))

- Decouple request parsing and command execution ([e0d7c67](https://github.com/elicore/redis-web/commit/e0d7c67b300ec8e99ce8f29e77e3fdde334fd2bb))


### Tests

- Cover db-prefix error-path edge cases  ([cca36b0](https://github.com/elicore/redis-web/commit/cca36b0a0c172a618b2cc0432ff3454cfd8af0e5))

- Cover uppercase percent-encoding parity  ([4dc7684](https://github.com/elicore/redis-web/commit/4dc768488b21e6344802559ffd9f9ad599b264bf))

- Split harness into unit/functional/integration tiers ([6664df1](https://github.com/elicore/redis-web/commit/6664df10b10f4ac8eb81cd79642e794b7d0860a1))

- Load legacy webdis fixture from docs examples ([d44ba9d](https://github.com/elicore/redis-web/commit/d44ba9d0df9f15079d403e316107ad09d6997107))

- Avoid codeql sensitive session_id flow in compat integration tests ([1b310f8](https://github.com/elicore/redis-web/commit/1b310f8bd506b6bdfb02b96fa038673024220cac))

- Increase compat session retry window for CI redis startup ([d430ea8](https://github.com/elicore/redis-web/commit/d430ea8d179ab015f1bed9b0ed0b52e53f459bdf))


### Ci

- Parameterize Redis image and use actions-rs toolchain ([c82e17b](https://github.com/elicore/redis-web/commit/c82e17b426511366acdca4c518f26541a74b6e18))

- *(workflow)* Use comma-separated TAGS env for build-push-action ([591e486](https://github.com/elicore/redis-web/commit/591e48691dd577b41343c46ebd4cc8045de52d17))

- Checkout repo before invoking local composite action ([5964917](https://github.com/elicore/redis-web/commit/5964917f2b1803fdc45c35b803d5bdf384971922))

- Fix workflow expression syntax and lint issues ([65bfcc6](https://github.com/elicore/redis-web/commit/65bfcc65feb0013bc59efb1a133b66a61a3f9451))

- Fix linux toolchain setup and avoid invalid macOS cross-target ([ed003c4](https://github.com/elicore/redis-web/commit/ed003c4c877fdd955102441f4b25881ec3bed18f))

- Drop unsupported macos-13 runner from matrix ([b76b056](https://github.com/elicore/redis-web/commit/b76b05606e92f5d67c1c8442a31892ab09c724ee))

- Simplify workflows and add local act validation ([6641634](https://github.com/elicore/redis-web/commit/66416343cba3a698475c0eab96e41bb412338fb1))

- Add release-please, changelog, and prek workflows ([deffc4c](https://github.com/elicore/redis-web/commit/deffc4ce38e864dac2d9c870e39635b095f41f1a))

- Skip docs or code jobs based on changes ([4643c6f](https://github.com/elicore/redis-web/commit/4643c6fec1c84f9c9b85ad881512aa550858ba6c))

- Limit PR checks to rust test/build and add crate publish workflow ([23dd663](https://github.com/elicore/redis-web/commit/23dd663194dab88338f3ee8ae2a8eb87f656a668))

- Expose redis service port and wait for readiness ([8264716](https://github.com/elicore/redis-web/commit/82647166258623d85aa4edf19c3d2aa5ab355a9d))


### Logging

- Add startup, redis validation, and execution error logs ([722646a](https://github.com/elicore/redis-web/commit/722646a6c316dabf8fbba45816cb01fb5c6b9c07))


### Root

- Fork point snapshot ([9a120cb](https://github.com/elicore/redis-web/commit/9a120cb778a0796b54ba6dbfcdb5ee0287c83473))



