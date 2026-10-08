# Changelog

## [1.5.1](https://github.com/jonerrr/trainstatus/compare/frontend-v1.5.0...frontend-v1.5.1) (2026-10-08)


### Bug Fixes

* map drag issues and improve overall ui/ux of map ([#439](https://github.com/jonerrr/trainstatus/issues/439)) ([1710fa7](https://github.com/jonerrr/trainstatus/commit/1710fa768a6f2f5b795f82259402354c43f6eb5e))

## [1.5.0](https://github.com/jonerrr/trainstatus/compare/frontend-v1.4.0...frontend-v1.5.0) (2026-10-08)


### Features

* **backend:** remove Redis and improve backend structure ([e1f1284](https://github.com/jonerrr/trainstatus/commit/e1f1284ed5fed9bb5980fd159565f8812965bc18))
* **csp:** add Content Security Policy configuration and related tests ([28349d9](https://github.com/jonerrr/trainstatus/commit/28349d9450cf9f89de338e5c50ab00420daa1e5e)), closes [#391](https://github.com/jonerrr/trainstatus/issues/391)


### Bug Fixes

* **csp:** update connect-src so maplibre works ([afe6e0b](https://github.com/jonerrr/trainstatus/commit/afe6e0bde48f5a50c89c1f200ec0bdaa4736275f))
* **List:** remove extra padding ([763bd9f](https://github.com/jonerrr/trainstatus/commit/763bd9f6854779a2e588ed530516b0c38f03bb48))
* **styles:** enhance scrollbar and selection styles ([8defac7](https://github.com/jonerrr/trainstatus/commit/8defac70de3bd45873bada81d56ed6fff976dd3d))

## [1.4.0](https://github.com/jonerrr/trainstatus/compare/frontend-v1.3.3...frontend-v1.4.0) (2026-10-07)


### Features

* **e2e:** add navigation and pins end-to-end tests ([bb298bc](https://github.com/jonerrr/trainstatus/commit/bb298bcd83ecd46eefb0fdf39cdd0e132e810ee0))


### Bug Fixes

* improve list scrolling behavior and height adjustment ([bef568e](https://github.com/jonerrr/trainstatus/commit/bef568ea0be15c8865c45ca5e5fe130998226210))
* PWA viewport issues ([e220dc6](https://github.com/jonerrr/trainstatus/commit/e220dc64f84839ab1074a55d1cdf7fe77627172a))
* settings page scroll ([eea6d46](https://github.com/jonerrr/trainstatus/commit/eea6d46b8b2d855b3768890fdee213f2546d65f6))
* various code improvements and bug fixes ([bb298bc](https://github.com/jonerrr/trainstatus/commit/bb298bcd83ecd46eefb0fdf39cdd0e132e810ee0))

## [1.3.3](https://github.com/jonerrr/trainstatus/compare/frontend-v1.3.2...frontend-v1.3.3) (2026-10-06)


### Bug Fixes

* remove inactive express routes from stop list and add tests for arrival logic ([95c2f96](https://github.com/jonerrr/trainstatus/commit/95c2f96bd7b82c2ed949ec3b22c0dad08d7a36f7)), closes [#410](https://github.com/jonerrr/trainstatus/issues/410)

## [1.3.2](https://github.com/jonerrr/trainstatus/compare/frontend-v1.3.1...frontend-v1.3.2) (2026-10-06)


### Bug Fixes

* chart stop ordering and add test suite for it ([86473b4](https://github.com/jonerrr/trainstatus/commit/86473b44fa705b512e20fa9188336171518ed1c0)), closes [#402](https://github.com/jonerrr/trainstatus/issues/402)
* update pnpm workspace configuration and enhance image handling ([ec0d8a0](https://github.com/jonerrr/trainstatus/commit/ec0d8a0f359f68d3f88289d0f1c973bb85b31070))

## [1.3.1](https://github.com/jonerrr/trainstatus/compare/frontend-v1.3.0...frontend-v1.3.1) (2026-10-05)


### Bug Fixes

* add API_ORIGIN config for SSR behind reverse proxy ([144fff4](https://github.com/jonerrr/trainstatus/commit/144fff433d49c7edb35ef2bca99710c885d9c1e5))

## [1.3.0](https://github.com/jonerrr/trainstatus/compare/frontend-v1.2.3...frontend-v1.3.0) (2026-10-05)


### Features

* Continuous tracking and large refactor ([#398](https://github.com/jonerrr/trainstatus/issues/398)) ([3e24cff](https://github.com/jonerrr/trainstatus/commit/3e24cff1bbe5e144a49d50ad78436f61b3aad15f))
* handle Android back gesture to manage modal closing ([7d96b6f](https://github.com/jonerrr/trainstatus/commit/7d96b6f915294bebc928e1b4d16c2028d5acca78)), closes [#255](https://github.com/jonerrr/trainstatus/issues/255)
* migrate to sveltekit 3 ([b2f04c4](https://github.com/jonerrr/trainstatus/commit/b2f04c4766d80985767172d9c785cefb32ff4574))
* use pnpm base image and fix demo SSR ([599faba](https://github.com/jonerrr/trainstatus/commit/599faba451937ca8a8370e82c731632c5f36defd)), closes [#401](https://github.com/jonerrr/trainstatus/issues/401)


### Bug Fixes

* **deps:** update frontend ([#363](https://github.com/jonerrr/trainstatus/issues/363)) ([831501d](https://github.com/jonerrr/trainstatus/commit/831501dac53c19a2f3ddf5fc89f478f99537f441))
* **docker:** make containers rootless ([b297e28](https://github.com/jonerrr/trainstatus/commit/b297e2852cee5f0ffe1eb536f0da9929d805160a)), closes [#337](https://github.com/jonerrr/trainstatus/issues/337)
* eslint setup and format frontend ([bc50475](https://github.com/jonerrr/trainstatus/commit/bc50475aa57b8a250f54b57906adea14b5f9a40b))
* modal flashing ([1027f1f](https://github.com/jonerrr/trainstatus/commit/1027f1fbdc7b1c1c46e234f0fa5b02e4756e749f)), closes [#285](https://github.com/jonerrr/trainstatus/issues/285)

## [1.2.3](https://github.com/jonerrr/trainstatus/compare/frontend-v1.2.2...frontend-v1.2.3) (2026-04-22)


### Bug Fixes

* **deps:** update frontend to fix twcss vite bug ([787f263](https://github.com/jonerrr/trainstatus/commit/787f263f1926304ff4e1f760bc14cc14461168c3))
* **mise:** run pnpm install if dep list changed when running dev task ([684a5b5](https://github.com/jonerrr/trainstatus/commit/684a5b505d7e675ce186b48534134f0ace14f1a0))

## [1.2.2](https://github.com/jonerrr/trainstatus/compare/frontend-v1.2.1...frontend-v1.2.2) (2026-04-22)


### Bug Fixes

* separate load function into universal and server side ([025a83d](https://github.com/jonerrr/trainstatus/commit/025a83dc51393958c2cea94b10e11bc1d8bf19bc))

## [1.2.1](https://github.com/jonerrr/trainstatus/compare/frontend-v1.2.0...frontend-v1.2.1) (2026-04-20)


### Bug Fixes

* **charts:** loading skeleton ([d7d8236](https://github.com/jonerrr/trainstatus/commit/d7d8236ece486263881d803f4d5378d2cd3a6c9f)), closes [#307](https://github.com/jonerrr/trainstatus/issues/307)
* trip share url not working ([e5f19d9](https://github.com/jonerrr/trainstatus/commit/e5f19d965353a2737b38c3cdaaa5031917937e32)), closes [#325](https://github.com/jonerrr/trainstatus/issues/325)
* unmonitor old routes ([7c8c1c7](https://github.com/jonerrr/trainstatus/commit/7c8c1c72cdf31676374bc15f627c6a3b08d8e030)), closes [#319](https://github.com/jonerrr/trainstatus/issues/319)

## [1.2.0](https://github.com/jonerrr/trainstatus/compare/frontend-v1.1.4...frontend-v1.2.0) (2026-04-11)


### Miscellaneous Chores

* **frontend:** Synchronize trainstatus versions

## [1.1.4](https://github.com/jonerrr/trainstatus/compare/frontend-v1.1.3...frontend-v1.1.4) (2026-03-29)


### Bug Fixes

* frontend docker build and put api client in frontend folder ([609a8c5](https://github.com/jonerrr/trainstatus/commit/609a8c5b165567980acdf2512688e4d403992153)), closes [#264](https://github.com/jonerrr/trainstatus/issues/264)

## [1.1.3](https://github.com/jonerrr/trainstatus/compare/frontend-v1.1.2...frontend-v1.1.3) (2026-03-29)


### Bug Fixes

* change localstorage keys ([f59e715](https://github.com/jonerrr/trainstatus/commit/f59e715a9cc5f3611c7ab3d881bbbd3593e6a122))
* **mta_bus:** get route geom from stop group polylines ([1d9e350](https://github.com/jonerrr/trainstatus/commit/1d9e350ac76bdf492208df42f3851bdf8193c7c8))

## [1.1.2](https://github.com/jonerrr/trainstatus/compare/frontend-v1.1.1...frontend-v1.1.2) (2026-03-25)


### Bug Fixes

* **map:** dont load maplibre css externally ([2193e58](https://github.com/jonerrr/trainstatus/commit/2193e58e3035cb98481a58e1a2b51c0ef9f2f484))

## [1.1.1](https://github.com/jonerrr/trainstatus/compare/frontend-v1.1.0...frontend-v1.1.1) (2026-03-24)


### Bug Fixes

* **charts:** add check that source should be monitored ([1295b8e](https://github.com/jonerrr/trainstatus/commit/1295b8ee9f6d392eea2fef7573c1e31de6ee3393))

## [1.1.0](https://github.com/jonerrr/trainstatus/compare/frontend-v1.0.2...frontend-v1.1.0) (2026-03-24)


### Features

* **api:** add dynamic API prefix configuration and update routes ([abf11bb](https://github.com/jonerrr/trainstatus/commit/abf11bb7d98e8801e53110174b97e1335307c722))

## [1.0.2](https://github.com/jonerrr/trainstatus/compare/frontend-v1.0.1...frontend-v1.0.2) (2026-03-23)


### Miscellaneous Chores

* **frontend:** Synchronize trainstatus versions

## [1.0.1](https://github.com/jonerrr/trainstatus/compare/frontend-v1.0.0...frontend-v1.0.1) (2026-03-23)


### Bug Fixes

* vite config ([3df6324](https://github.com/jonerrr/trainstatus/commit/3df63240e3cd0c9ade889235ec1a757f058230e3))

## 1.0.0 (2026-03-23)


### Features

* rewrite ([#222](https://github.com/jonerrr/trainstatus/issues/222)) ([0bf46a7](https://github.com/jonerrr/trainstatus/commit/0bf46a74933432415696d1c57b3c69e1e5ce9363))


### Bug Fixes

* frontend docker image and test compose stack ([e0bedff](https://github.com/jonerrr/trainstatus/commit/e0bedff9976cd2535806957b5a49ef6a0203fe3c))
* frontend docker node version ([bc20df4](https://github.com/jonerrr/trainstatus/commit/bc20df411ff4b87178f2c66ca37e3dcad80a6cbf))
