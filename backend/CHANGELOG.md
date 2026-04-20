# Changelog

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
