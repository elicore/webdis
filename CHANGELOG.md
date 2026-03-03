# Changelog

All notable changes to this project will be documented in this file.
## [Unreleased]

### #89

- Add Authorization to CORS headers ([8b1e91a](https://github.com/elicore/redis-web/commit/8b1e91a2e5b49ee5409b4f6181be70e0f29cf43c))


### #91

- Add missing return after sending HTTP Options ([f69c28c](https://github.com/elicore/redis-web/commit/f69c28c632800567ab6fc94d17e4cf1073cea1ba))


### BUG

- Adjust order of memory release ([7b7b51a](https://github.com/elicore/redis-web/commit/7b7b51af5f6eda85a5e30305e9f3ba7ec94925ee))


### CodeQL

- Apply recommendations ([b3868d8](https://github.com/elicore/redis-web/commit/b3868d81d891e3a3ea8304c059d33ddc8885cbbc))

- Potentially uninitialized local variable ([d2b1156](https://github.com/elicore/redis-web/commit/d2b115616f36287caa2e9d59b54c8b090613b6a9))

- Poorly documented large function ([3aaeabf](https://github.com/elicore/redis-web/commit/3aaeabfcd316e223a713b7c521b9b52785da0f0d))


### Documentation

- Rewrite README for Rust implementation and configuration ([f7e5245](https://github.com/elicore/redis-web/commit/f7e524578883537a7ce08ef806c61a045c7181fe))

- Add embedding guides and interface reference ([5e5aeae](https://github.com/elicore/redis-web/commit/5e5aeae3dc2068f8a3d8bb87cbd07401e8accdde))

- Add AGENTS workflow and command guide ([0f53b7a](https://github.com/elicore/redis-web/commit/0f53b7ab5763ff073782c2d234b08aa483224e38))

- Migrate to Starlight and add migration/deprecation guidance ([d581a1d](https://github.com/elicore/redis-web/commit/d581a1d305c99297b20afdaba0ddc3afa1758e1e))


### Features

- Transition CI to Rust-based build and test, update GitHub Actions matrix with newer Ubuntu, Red Hat, and macOS versions, and upgrade Redis to 8.0. ([cf02c80](https://github.com/elicore/redis-web/commit/cf02c8080ef2d05e789e1066e1d7945355d78966))

- *(config)* Add schema-backed defaults and config generator ([6214996](https://github.com/elicore/redis-web/commit/62149968cbf32ef2f7f50f79e215159e6c9653d9))

- Add composite actions for Rust setup and build/test workflow ([407a204](https://github.com/elicore/redis-web/commit/407a2042bf4158a81ca6699d18a4efb39c67b9b8))

- Add workflow for building and deploying Astro site to GitHub Pages ([38e6cff](https://github.com/elicore/redis-web/commit/38e6cff5ba0e6b2cbddab5190c9e26729cd4d0ad))

- *(docs)* Replace 'nicolas/webdis' with 'elicore/webdis' across docs and add docker docs ([79d756c](https://github.com/elicore/redis-web/commit/79d756c8c4ad036963a0f8f5f11a6ba4c0aaf8dd))

- Implement ETag support for GET requests and add feature list to README  ([9f0b895](https://github.com/elicore/redis-web/commit/9f0b8956fedf8548599ec3e85ff135afb1190e62))

- Implement WebSocket raw RESP endpoint at /.raw   ([cb5afa3](https://github.com/elicore/redis-web/commit/cb5afa3655b53f1a86a41e8a2694c51339dfc321))

- Implement structured JSON output for INFO command  ([1ad16c0](https://github.com/elicore/redis-web/commit/1ad16c01b5f8e28cf55397dd80aa411d09dcfffd))

- True raw RESP output for .raw endpoint  ([b587d4f](https://github.com/elicore/redis-web/commit/b587d4f5bdfed89859f4608b73f9bc1d0459bb22))

- Upgrade Axum to 0.8 and redis-rs to 0.32, adapting to new routing syntax, WebSocket text types, and RESP3 value variants. ([bf20c37](https://github.com/elicore/redis-web/commit/bf20c37a425143e71e23163dfd9c72a73f83b0de))

- Add JSONP support for HTTP responses  ([420e424](https://github.com/elicore/redis-web/commit/420e424d5807d175ef769a02e7c96334f7bc1b5a))

- Expand $VARNAME placeholders in JSON config  ([4eaccf5](https://github.com/elicore/redis-web/commit/4eaccf5e6faf2f933db235257a7ba033a3f4f37a))

- Honor hiredis.keep_alive_sec for Redis TCP keep-alive  ([6dd06b5](https://github.com/elicore/redis-web/commit/6dd06b5568064212f670460f9a7d86094b805d7b))

- URL encoding for `%2f` and `%2e` in command segments  ([4033bf0](https://github.com/elicore/redis-web/commit/4033bf0a55cd095271330b178db98c77f444ef5b))

- Implement log_fsync durability modes  ([deb1dc5](https://github.com/elicore/redis-web/commit/deb1dc50a25bf06fffc797457f0836421d06757f))

- Add chunked HTTP Pub/Sub JSON/JSONP parity   ([a12b946](https://github.com/elicore/redis-web/commit/a12b9468741492eb87e6333ecce18ee1ffd6ba39))

- *(parity)* Database selection via /<db>/ prefix   ([4102a92](https://github.com/elicore/redis-web/commit/4102a9281d767853d639929ac9c5210930ed76b0))

- Split into redis-web workspace crates with compatibility layer ([4950525](https://github.com/elicore/redis-web/commit/49505256de2ae7e7250e7414506547c7eb2ed9b4))


### GHA

- Upgrade Ubuntu, include Websocket tests ([9a29a85](https://github.com/elicore/redis-web/commit/9a29a85a944b3ddf52f4b9458edfc696cff1d5a6))


### Miscellaneous Tasks

- Remove CodeQL workflow configuration ([243ab57](https://github.com/elicore/redis-web/commit/243ab57b8454c7637eefae1e45dd66275a95baed))

- Update workflow triggers to only allow manual dispatch ([76671b9](https://github.com/elicore/redis-web/commit/76671b9151b624c56b09fdbec998178a1bc158e8))

- *(scripts)* Make helper scripts executable ([37bb241](https://github.com/elicore/redis-web/commit/37bb24107ff5ecaf56f22937ee60041b7260d6a8))

- *(make)* Remove docker helper targets per request ([3b49f8d](https://github.com/elicore/redis-web/commit/3b49f8d374b077223b288541e15a4116b15e3222))

- Update redis to 1.0.3, cleanup makefile ([369352a](https://github.com/elicore/redis-web/commit/369352aa8df3293598eb7c6e215a16f1590cbd7d))

- Add json schema to config examples ([71538fe](https://github.com/elicore/redis-web/commit/71538fec826edee90ce86155ac6c48b93c8e876b))

- *(deps)* Bump time from 0.3.44 to 0.3.47  ([970067c](https://github.com/elicore/redis-web/commit/970067c4c838f44d07e471237ba631e42331e8bd))

- Update runtime ops, compose, and CI workflows for redis-web ([000e9b7](https://github.com/elicore/redis-web/commit/000e9b7a551f59953904e975ce9b61f46508d9c4))

- Move prek to local developer config ([c0c3d91](https://github.com/elicore/redis-web/commit/c0c3d9101c45d0721d2d844f942f92a1ffdacd3b))

- Relocate docker assets and config examples to docs ([cd8df8d](https://github.com/elicore/redis-web/commit/cd8df8d33edfc2c4f64c494e508330c8b3deb534))


### Parity

- Redis UNIX socket support  ([071b208](https://github.com/elicore/redis-web/commit/071b208ea2bc6d7cd15132d90b3481e482068472))

- Custom content types and ?type= MIME overrides  ([95a7d04](https://github.com/elicore/redis-web/commit/95a7d0487d9f414519a09d5be9aa66bd3cdf6f85))


### README

- Add AWS ECR links, clean up Markdown ([09f0ccc](https://github.com/elicore/redis-web/commit/09f0ccc355db4034cbe91c1f0ef388cb93763b83))

- Document WebSocket demo, add links, minor cleanup ([b1b300f](https://github.com/elicore/redis-web/commit/b1b300f508930fc71fd2972bafafb33361a6469e))


### Refactor

- Move tests for parse_info_output to a dedicated test file ([d192743](https://github.com/elicore/redis-web/commit/d1927433ef56fa2684a9e6fb75b2ce16d2ac93b5))


### Refactoring

- Migrate build system to Cargo and improve integration tests with dynamic port allocation. ([36c578d](https://github.com/elicore/redis-web/commit/36c578d6116a55c13e21fbc8fe334a9b8284b939))

- Remove legacy Python/C tests and related files, update Rust tests, and add a new README. ([0fa63cc](https://github.com/elicore/redis-web/commit/0fa63cc899c4b9349594136dc6a7ef9dbde2972a))

- Decouple request parsing and command execution ([157a7e6](https://github.com/elicore/redis-web/commit/157a7e6ceeb3b2ec2262bf6956e01e1bae61eb93))


### TODO

- S/evhttp/libevent+http_parser/g ([85f38a0](https://github.com/elicore/redis-web/commit/85f38a023ac60178ef8972ecbdf623b8cfc4dc8c))


### Tests

- Cover db-prefix error-path edge cases  ([a893ac7](https://github.com/elicore/redis-web/commit/a893ac7aba3d596761243751481e50e7bd2de89b))

- Cover uppercase percent-encoding parity  ([2736094](https://github.com/elicore/redis-web/commit/2736094c706341ac894bb18b5b72b414e702eebc))

- Cover uppercase percent-encoding parity ([ae34b7f](https://github.com/elicore/redis-web/commit/ae34b7f438ac00876e3ded22383a388883508b4e))

- Split harness into unit/functional/integration tiers ([5d03814](https://github.com/elicore/redis-web/commit/5d0381468452f16bbae0b5692bf0d2b9baaccd0f))

- Load legacy webdis fixture from docs examples ([95e6823](https://github.com/elicore/redis-web/commit/95e6823dd0ee041abebcea635390da4bd4a1a9cc))


### WS

- Better reuse of the cmd struct for WS clients ([e26d635](https://github.com/elicore/redis-web/commit/e26d6358e7a216427a88952b52a0dc4302a739fa))

- Log commands ([dedfc42](https://github.com/elicore/redis-web/commit/dedfc42c676421338e72bba83a6b4857fb9715cb))


### Ci

- Parameterize Redis image and use actions-rs toolchain ([48733c8](https://github.com/elicore/redis-web/commit/48733c88ce6b4a1c5e3e717dac9ef4bd74c8aa04))

- *(workflow)* Use comma-separated TAGS env for build-push-action ([c94b3ea](https://github.com/elicore/redis-web/commit/c94b3ea4071f0b300f427ad90fd972d015f37bc1))

- Checkout repo before invoking local composite action ([95b93c4](https://github.com/elicore/redis-web/commit/95b93c4d35684c5e507318efe9506826bab90122))

- Fix workflow expression syntax and lint issues ([2ebb6ab](https://github.com/elicore/redis-web/commit/2ebb6abde26026f5ab8cd2974889e4f5bf33b65e))

- Fix linux toolchain setup and avoid invalid macOS cross-target ([816e3c6](https://github.com/elicore/redis-web/commit/816e3c6c6fdab8146267c95284761a7099714e98))

- Drop unsupported macos-13 runner from matrix ([2204230](https://github.com/elicore/redis-web/commit/2204230fb0960d344597b4424fc442496bcfde0b))

- Simplify workflows and add local act validation ([6512080](https://github.com/elicore/redis-web/commit/6512080457b8c1d296c2903a0cd74ecafb2d490b))

- Add release-please, changelog, and prek workflows ([1adbd6f](https://github.com/elicore/redis-web/commit/1adbd6f4f91968043874af397187fdf9834da284))


### Slog.c

- Change level symbol to a single letter ([09bd76f](https://github.com/elicore/redis-web/commit/09bd76f3a80eda63f21efd9cdca444ae72e65e14))



