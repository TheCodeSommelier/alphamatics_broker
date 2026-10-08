# Changelog


## [v0.18.0](https://github.com/TheCodeSommelier/alphamatics_broker/compare/v0.17.0...v0.18.0)

* Merge pull request #21 from TheCodeSommelier/feature/serialize-rfid-as-string [[41cc37021deb3e80dbaf24123e28eb0daad94905](https://github.com/TheCodeSommelier/alphamatics_broker/commit/41cc37021deb3e80dbaf24123e28eb0daad94905)]
* feat: serialize rfid as a string [[7e2e603886cad96471d1b59309dbb4643b7e5253](https://github.com/TheCodeSommelier/alphamatics_broker/commit/7e2e603886cad96471d1b59309dbb4643b7e5253)]


## [v0.17.0](https://github.com/TheCodeSommelier/alphamatics_broker/compare/v0.1.0...v0.17.0)

* Merge pull request #20 from TheCodeSommelier/stage [[da6720c81423c874a739f301e27d6d53dd4e32b6](https://github.com/TheCodeSommelier/alphamatics_broker/commit/da6720c81423c874a739f301e27d6d53dd4e32b6)]
* Merge pull request #19 from TheCodeSommelier/feature/prod-ready [[f3ce8a009c02be882bb8af546a76374e2e8e9096](https://github.com/TheCodeSommelier/alphamatics_broker/commit/f3ce8a009c02be882bb8af546a76374e2e8e9096)]
* feat: structured logging and self-restarting command listener [[b12f89deddb292617f1f007a7945579438e7aae1](https://github.com/TheCodeSommelier/alphamatics_broker/commit/b12f89deddb292617f1f007a7945579438e7aae1)]
* feat: fail stuck commands instead of retrying forever [[6e6759c1b0b503710dfaada51b6f35c734539b53](https://github.com/TheCodeSommelier/alphamatics_broker/commit/6e6759c1b0b503710dfaada51b6f35c734539b53)]
* feat: harden device connections for production [[a6e6160baff5709e9ebd18ccdeebde771129264a](https://github.com/TheCodeSommelier/alphamatics_broker/commit/a6e6160baff5709e9ebd18ccdeebde771129264a)]
* fix: bind NATS monitoring port to loopback [[5acb22a4912f39b887833a3372f65e22456d7dbb](https://github.com/TheCodeSommelier/alphamatics_broker/commit/5acb22a4912f39b887833a3372f65e22456d7dbb)]
* fix: wait for JetStream ack before acking device frames [[9206937f7c874ee277374d8b4a02e6e239051757](https://github.com/TheCodeSommelier/alphamatics_broker/commit/9206937f7c874ee277374d8b4a02e6e239051757)]
* feat: add sladg release utils [[75eb80032835ef83ca780c7baef0b230b0e63aea](https://github.com/TheCodeSommelier/alphamatics_broker/commit/75eb80032835ef83ca780c7baef0b230b0e63aea)]
* Merge pull request #16 from TheCodeSommelier/feature/rfid-enrollment [[008b521023a40482fa3b1a00b7cee76f517a174d](https://github.com/TheCodeSommelier/alphamatics_broker/commit/008b521023a40482fa3b1a00b7cee76f517a174d)]
* feat: fix deployments [[b2e94b10cfd3b06d560792d8e03aaeefa954dbc6](https://github.com/TheCodeSommelier/alphamatics_broker/commit/b2e94b10cfd3b06d560792d8e03aaeefa954dbc6)]
* chore: fix typing [[441d8974f080051b68650c3ddb475bc6a38bbb0a](https://github.com/TheCodeSommelier/alphamatics_broker/commit/441d8974f080051b68650c3ddb475bc6a38bbb0a)]
* feat: add rfid enrollment [[35695141397233d897c1a3a37fb2ad83081ce3e9](https://github.com/TheCodeSommelier/alphamatics_broker/commit/35695141397233d897c1a3a37fb2ad83081ce3e9)]
* Merge pull request #15 from TheCodeSommelier/feature/pre-release-fixes [[3892b3b81816e2ddc9783d3803cfdd6aaf05dd5d](https://github.com/TheCodeSommelier/alphamatics_broker/commit/3892b3b81816e2ddc9783d3803cfdd6aaf05dd5d)]
* feat: add pre release check [[8dbd6fd724583edc8229fc568c09dc449dfe3622](https://github.com/TheCodeSommelier/alphamatics_broker/commit/8dbd6fd724583edc8229fc568c09dc449dfe3622)]
* feat: stop exesive logging [[5186f9f9b6c8682b6e5c18f772f940a6fadb6f15](https://github.com/TheCodeSommelier/alphamatics_broker/commit/5186f9f9b6c8682b6e5c18f772f940a6fadb6f15)]
* Merge pull request #14 from TheCodeSommelier/feature/send-commands [[94cbf052a9cb06d9c55ec0e9faf3fb331bef50c9](https://github.com/TheCodeSommelier/alphamatics_broker/commit/94cbf052a9cb06d9c55ec0e9faf3fb331bef50c9)]
* feat: add reids queue [[6103e90c1df640ca5497ad9dd20bf57361b266ae](https://github.com/TheCodeSommelier/alphamatics_broker/commit/6103e90c1df640ca5497ad9dd20bf57361b266ae)]
* feat: add redis [[b47094d435f809d95be19e7ea3f5c61524bd50d3](https://github.com/TheCodeSommelier/alphamatics_broker/commit/b47094d435f809d95be19e7ea3f5c61524bd50d3)]
* feat: implement the codec 12 protocol [[df2d4f66dc93f38c06de2824bd980ece602afb0a](https://github.com/TheCodeSommelier/alphamatics_broker/commit/df2d4f66dc93f38c06de2824bd980ece602afb0a)]
* Merge pull request #13 from TheCodeSommelier/feature/validations [[28796cfd43cdda824a02f711c7cb28168e195de0](https://github.com/TheCodeSommelier/alphamatics_broker/commit/28796cfd43cdda824a02f711c7cb28168e195de0)]
* feat: add timestamp validations [[8579a430ac8d3764552202c569a1843bb75b0b2f](https://github.com/TheCodeSommelier/alphamatics_broker/commit/8579a430ac8d3764552202c569a1843bb75b0b2f)]
* Merge pull request #12 from TheCodeSommelier/feature/deploy-aws [[499b9423aec413716e94fb11d09b1908502d1fa9](https://github.com/TheCodeSommelier/alphamatics_broker/commit/499b9423aec413716e94fb11d09b1908502d1fa9)]
* feat: add env to sentry [[082ba32ea7032254ed7bbbfa5105b76c1d0aee3e](https://github.com/TheCodeSommelier/alphamatics_broker/commit/082ba32ea7032254ed7bbbfa5105b76c1d0aee3e)]
* Merge pull request #11 from TheCodeSommelier/feature/deploy-aws [[4bd1ece83adc2f56a72d58234317bc561c4b7527](https://github.com/TheCodeSommelier/alphamatics_broker/commit/4bd1ece83adc2f56a72d58234317bc561c4b7527)]
* feat: add the .image.env file to broker deploy [[88bc35d97fed41b8e113e33d641cc39e901f2f6b](https://github.com/TheCodeSommelier/alphamatics_broker/commit/88bc35d97fed41b8e113e33d641cc39e901f2f6b)]
* Merge pull request #10 from TheCodeSommelier/feature/deploy-aws [[6affbd0d21e521eaf37c1b3e6cfa7e75604991fb](https://github.com/TheCodeSommelier/alphamatics_broker/commit/6affbd0d21e521eaf37c1b3e6cfa7e75604991fb)]
* feat: add the correct image name in docker compose [[da5dfc13d8bc292ef89ac69fb3fc467564b7e7ef](https://github.com/TheCodeSommelier/alphamatics_broker/commit/da5dfc13d8bc292ef89ac69fb3fc467564b7e7ef)]
* Merge pull request #8 from TheCodeSommelier/feature/deploy-aws [[29fc0366af81ca2d30d000ec6f539b873fcd474e](https://github.com/TheCodeSommelier/alphamatics_broker/commit/29fc0366af81ca2d30d000ec6f539b873fcd474e)]
* feat: add an environment to the github action [[4e40024be0a97bd2c8b1744a08d9759045c06d14](https://github.com/TheCodeSommelier/alphamatics_broker/commit/4e40024be0a97bd2c8b1744a08d9759045c06d14)]


## [v0.1.0](https://github.com/TheCodeSommelier/alphamatics_broker/commits/v0.1.0)

* Merge pull request #6 from TheCodeSommelier/feature/deploy-aws [[82ccf7ce9667512d3b3f328bb9d1d71db7ac1249](https://github.com/TheCodeSommelier/alphamatics_broker/commit/82ccf7ce9667512d3b3f328bb9d1d71db7ac1249)]
* feat: add docker file, docker compose, release-plz and github actions [[ae37b59dd15d2e4d9470f7dc57ab44d29d00d1b8](https://github.com/TheCodeSommelier/alphamatics_broker/commit/ae37b59dd15d2e4d9470f7dc57ab44d29d00d1b8)]
* Merge pull request #5 from TheCodeSommelier/feature/nats-stream [[9ab233f8922813bcb993f6f79dde526982ed10f3](https://github.com/TheCodeSommelier/alphamatics_broker/commit/9ab233f8922813bcb993f6f79dde526982ed10f3)]
* feat: update data modelling && nats subject publishing [[9dbc0c29816ab4f86bf756ea0505eb61326c2ae9](https://github.com/TheCodeSommelier/alphamatics_broker/commit/9dbc0c29816ab4f86bf756ea0505eb61326c2ae9)]
* feat: add imei to data sent through nats [[3327ea0bee68b5d9a419733d10e2447ce840c76f](https://github.com/TheCodeSommelier/alphamatics_broker/commit/3327ea0bee68b5d9a419733d10e2447ce840c76f)]
* feat: add nats streaming [[5650620942c419c90348ed59ad38d92169203a3e](https://github.com/TheCodeSommelier/alphamatics_broker/commit/5650620942c419c90348ed59ad38d92169203a3e)]
* Merge pull request #4 from TheCodeSommelier/feature/db [[ea9bc5133700173aa31c9759f26200f745c9c7f4](https://github.com/TheCodeSommelier/alphamatics_broker/commit/ea9bc5133700173aa31c9759f26200f745c9c7f4)]
* feat: add proper db pooling [[dccb473a11d8a1c826e3de1c4c3bd01f4c5a2f4b](https://github.com/TheCodeSommelier/alphamatics_broker/commit/dccb473a11d8a1c826e3de1c4c3bd01f4c5a2f4b)]
* feat: generate schema by diesel [[6cff5ca4179b549828220e7e8e3d30d18500345d](https://github.com/TheCodeSommelier/alphamatics_broker/commit/6cff5ca4179b549828220e7e8e3d30d18500345d)]
* feat: add DB with diesel ORM [[c8d9941128eec17ba06125fa0f07d5c1645f42fb](https://github.com/TheCodeSommelier/alphamatics_broker/commit/c8d9941128eec17ba06125fa0f07d5c1645f42fb)]
* Merge pull request #3 from TheCodeSommelier/feature/dotenvy [[9decc262424a2ef0ae183b9d3a052e30a7520461](https://github.com/TheCodeSommelier/alphamatics_broker/commit/9decc262424a2ef0ae183b9d3a052e30a7520461)]
* feat: wire up dotenvy [[755de6bdb12da91614180bb56b07299f34d90fe7](https://github.com/TheCodeSommelier/alphamatics_broker/commit/755de6bdb12da91614180bb56b07299f34d90fe7)]
* Stop tracking .env [[578cb2ebb6e8c04b8a2c1ab5f49a971eceb3958c](https://github.com/TheCodeSommelier/alphamatics_broker/commit/578cb2ebb6e8c04b8a2c1ab5f49a971eceb3958c)]
* Merge pull request #2 from TheCodeSommelier/feature/add-tcp-listener [[4eb19c2389b0f7a13c4b30a32f6599c9ba0d5fd2](https://github.com/TheCodeSommelier/alphamatics_broker/commit/4eb19c2389b0f7a13c4b30a32f6599c9ba0d5fd2)]
* feat: clean up commit [[2e5872d83839290917cc74dbbdf7c5a23199970d](https://github.com/TheCodeSommelier/alphamatics_broker/commit/2e5872d83839290917cc74dbbdf7c5a23199970d)]
* feat: add .env to git ignore [[f6eca957395259f76825be3e9ffe871135e1617c](https://github.com/TheCodeSommelier/alphamatics_broker/commit/f6eca957395259f76825be3e9ffe871135e1617c)]
* Merge pull request #1 from TheCodeSommelier/feature/add-tcp-listener [[b5e24249d0add2fe0ac11e886c3677ddfde201fe](https://github.com/TheCodeSommelier/alphamatics_broker/commit/b5e24249d0add2fe0ac11e886c3677ddfde201fe)]
* feat: make sentry log the errors and continue to listen to tcp stream [[9e3a7d84c6c9d0f6d71b1de1a0669f2967b13fbc](https://github.com/TheCodeSommelier/alphamatics_broker/commit/9e3a7d84c6c9d0f6d71b1de1a0669f2967b13fbc)]
* feat: add sentry [[b019fabca78fbc894c5b9466893ddfc39bdf7076](https://github.com/TheCodeSommelier/alphamatics_broker/commit/b019fabca78fbc894c5b9466893ddfc39bdf7076)]
* refactor: remove old comments [[ae2719ad212978739d22b6fb5082bb6f18ea34e3](https://github.com/TheCodeSommelier/alphamatics_broker/commit/ae2719ad212978739d22b6fb5082bb6f18ea34e3)]
* feat: parse an number of bytes in the AVL [[acfbfddc0a7153ee89d4b6c32c4f72034b43fe05](https://github.com/TheCodeSommelier/alphamatics_broker/commit/acfbfddc0a7153ee89d4b6c32c4f72034b43fe05)]
* feat: add tcp listener && parser [[684767ba7950bcbce5154046fc453b9b330a8732](https://github.com/TheCodeSommelier/alphamatics_broker/commit/684767ba7950bcbce5154046fc453b9b330a8732)]
* feat: init [[97bb46a35ad5b5d510cde78a61d6ca9c7bafae7d](https://github.com/TheCodeSommelier/alphamatics_broker/commit/97bb46a35ad5b5d510cde78a61d6ca9c7bafae7d)]