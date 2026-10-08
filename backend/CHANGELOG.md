# Changelog

## [1.5.0](https://github.com/jonerrr/trainstatus/compare/backend-v1.4.0...backend-v1.5.0) (2026-10-08)


### Features

* **backend:** remove Redis and improve backend structure ([e1f1284](https://github.com/jonerrr/trainstatus/commit/e1f1284ed5fed9bb5980fd159565f8812965bc18))
* **health:** add liveness and readiness endpoints with tests ([f06917f](https://github.com/jonerrr/trainstatus/commit/f06917f9709cfa15502ce82d553970a81ebec7d5)), closes [#378](https://github.com/jonerrr/trainstatus/issues/378)


### Bug Fixes

* **api:** bring utoapi docs up to date ([1600b0f](https://github.com/jonerrr/trainstatus/commit/1600b0ffb6f8c72a3aff388041dcc6259833858e))
* **backend:** update fixtures ([83e0763](https://github.com/jonerrr/trainstatus/commit/83e07630a39e5e149714ffcd48d1176a395538c1))
* **Dockerfile:** ensure /app directory ownership and set user for backend ([e147a85](https://github.com/jonerrr/trainstatus/commit/e147a85a3e49f00a6b637e8622b507d1cd70a754))

## [1.4.0](https://github.com/jonerrr/trainstatus/compare/backend-v1.3.3...backend-v1.4.0) (2026-10-07)


### Miscellaneous Chores

* **backend:** Synchronize trainstatus versions

## [1.3.3](https://github.com/jonerrr/trainstatus/compare/backend-v1.3.2...backend-v1.3.3) (2026-10-06)


### Miscellaneous Chores

* **backend:** Synchronize trainstatus versions

## [1.3.2](https://github.com/jonerrr/trainstatus/compare/backend-v1.3.1...backend-v1.3.2) (2026-10-06)


### Bug Fixes

* add RUST_TEST_THREADS environment variable ([04a9276](https://github.com/jonerrr/trainstatus/commit/04a92762310547e906d7a3c300916df5b075a1b3))

## [1.3.1](https://github.com/jonerrr/trainstatus/compare/backend-v1.3.0...backend-v1.3.1) (2026-10-05)


### Miscellaneous Chores

* **backend:** Synchronize trainstatus versions

## [1.3.0](https://github.com/jonerrr/trainstatus/compare/backend-v1.2.3...backend-v1.3.0) (2026-10-05)


### Features

* Continuous tracking and large refactor ([#398](https://github.com/jonerrr/trainstatus/issues/398)) ([3e24cff](https://github.com/jonerrr/trainstatus/commit/3e24cff1bbe5e144a49d50ad78436f61b3aad15f))
* use sqlx toml config to override types ([453eb56](https://github.com/jonerrr/trainstatus/commit/453eb56620907029f3737f240f1ccc9d47c0ac28))


### Bug Fixes

* **deps:** update arrow to version 60.0.0 and axum-test to version 21.1.0 ([e08836d](https://github.com/jonerrr/trainstatus/commit/e08836dde5946e3c090da44e00d4478f2013ea02))
* **deps:** update backend ([#362](https://github.com/jonerrr/trainstatus/issues/362)) ([4a92e19](https://github.com/jonerrr/trainstatus/commit/4a92e19dfdf7de0e6665d508307582294d1e9463))
* **docker:** make containers rootless ([b297e28](https://github.com/jonerrr/trainstatus/commit/b297e2852cee5f0ffe1eb536f0da9929d805160a)), closes [#337](https://github.com/jonerrr/trainstatus/issues/337)
* remove cors since its no longer needed ([924c588](https://github.com/jonerrr/trainstatus/commit/924c58843ad3cd2b3aa5fb7080412c0693c52f9e))
* update geozero usage and sqlx features for 0.9.x update ([4a8d68d](https://github.com/jonerrr/trainstatus/commit/4a8d68d4778ed5da57df8c98e0271cd0b8a8a5a1))

## [1.2.3](https://github.com/jonerrr/trainstatus/compare/backend-v1.2.2...backend-v1.2.3) (2026-04-22)


### Miscellaneous Chores

* **backend:** Synchronize trainstatus versions

## [1.2.2](https://github.com/jonerrr/trainstatus/compare/backend-v1.2.1...backend-v1.2.2) (2026-04-22)


### Miscellaneous Chores

* **backend:** Synchronize trainstatus versions

## [1.2.1](https://github.com/jonerrr/trainstatus/compare/backend-v1.2.0...backend-v1.2.1) (2026-04-20)


### Bug Fixes

* **deps:** update backend ([#318](https://github.com/jonerrr/trainstatus/issues/318)) ([197d6a3](https://github.com/jonerrr/trainstatus/commit/197d6a342941d358a881c7d42ea4a778bba3ee0b))
* **mta_subway:** update geometry handling to support multiple features per route ([de28839](https://github.com/jonerrr/trainstatus/commit/de28839a1282be4a3a72fc4571f22ca64c3b3d87))

## [1.2.0](https://github.com/jonerrr/trainstatus/compare/backend-v1.1.4...backend-v1.2.0) (2026-04-11)


### Features

* **mta_subway:** add route geometry ([8c219f9](https://github.com/jonerrr/trainstatus/commit/8c219f98c3f20f9866a6bca9f8c7698dc1b00c0a)), closes [#271](https://github.com/jonerrr/trainstatus/issues/271)


### Bug Fixes

* various clippy warnings ([9f648be](https://github.com/jonerrr/trainstatus/commit/9f648beb64c18b10a6bbbc1b098bcd67f636f905))

## [1.1.4](https://github.com/jonerrr/trainstatus/compare/backend-v1.1.3...backend-v1.1.4) (2026-03-29)


### Bug Fixes

* frontend docker build and put api client in frontend folder ([609a8c5](https://github.com/jonerrr/trainstatus/commit/609a8c5b165567980acdf2512688e4d403992153)), closes [#264](https://github.com/jonerrr/trainstatus/issues/264)

## [1.1.3](https://github.com/jonerrr/trainstatus/compare/backend-v1.1.2...backend-v1.1.3) (2026-03-29)


### Bug Fixes

* **mta_bus:** get route geom from stop group polylines ([1d9e350](https://github.com/jonerrr/trainstatus/commit/1d9e350ac76bdf492208df42f3851bdf8193c7c8))

## [1.1.2](https://github.com/jonerrr/trainstatus/compare/backend-v1.1.1...backend-v1.1.2) (2026-03-25)


### Miscellaneous Chores

* **backend:** Synchronize trainstatus versions

## [1.1.1](https://github.com/jonerrr/trainstatus/compare/backend-v1.1.0...backend-v1.1.1) (2026-03-24)


### Miscellaneous Chores

* **backend:** Synchronize trainstatus versions

## [1.1.0](https://github.com/jonerrr/trainstatus/compare/backend-v1.0.2...backend-v1.1.0) (2026-03-24)


### Features

* **api:** add dynamic API prefix configuration and update routes ([abf11bb](https://github.com/jonerrr/trainstatus/commit/abf11bb7d98e8801e53110174b97e1335307c722))

## [1.0.2](https://github.com/jonerrr/trainstatus/compare/backend-v1.0.1...backend-v1.0.2) (2026-03-23)


### Bug Fixes

* remove backend compose.yml and itertools dep ([e94bc1f](https://github.com/jonerrr/trainstatus/commit/e94bc1f7f164a5bbc3b9b463c0253d18fa267870))

## [1.0.1](https://github.com/jonerrr/trainstatus/compare/backend-v1.0.0...backend-v1.0.1) (2026-03-23)


### Miscellaneous Chores

* **backend:** Synchronize trainstatus versions

## 1.0.0 (2026-03-23)


### Features

* rewrite ([#222](https://github.com/jonerrr/trainstatus/issues/222)) ([0bf46a7](https://github.com/jonerrr/trainstatus/commit/0bf46a74933432415696d1c57b3c69e1e5ce9363))


### Bug Fixes

* **deps:** update rust crate geojson to v1 ([#239](https://github.com/jonerrr/trainstatus/issues/239)) ([ed6de3e](https://github.com/jonerrr/trainstatus/commit/ed6de3e008f39eebb987654135341bb50c402a4e))
* frontend docker image and test compose stack ([e0bedff](https://github.com/jonerrr/trainstatus/commit/e0bedff9976cd2535806957b5a49ef6a0203fe3c))
* geometryValue enum ([9e51000](https://github.com/jonerrr/trainstatus/commit/9e51000b5997a75f9d2ff7de723588bc1de1a168))
* match njt bus geometry using BUSDV2 api + route long name ([6467518](https://github.com/jonerrr/trainstatus/commit/646751839f0654e18b36d554a91a6871ea90a084))
* **njt_bus:** use LINE property instead of LINESTRING property ([303f152](https://github.com/jonerrr/trainstatus/commit/303f15236d6311c77e21640612efd2751c60ca7f))
* replace scratch image with valhalla ([fed3069](https://github.com/jonerrr/trainstatus/commit/fed3069be583bd8192a662cf417994f0daf58470))
