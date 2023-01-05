# Changelog

<!-- next-header -->

## [Unreleased] - ReleaseDate

### Added

- Add 'My feed' to i18n timeline name (#1084)
- Bidirectional support for user page header (#1092)

### Changed

- Use blog title as slug (#1094, #1126, #1127)
- Bump Rust to nightly 2022-07-19 (#1119)

### Fixed

- Malfunction while creating a blog post in Persian (#1116)
- Email block list is ignored when email sign-up (#1122)
- Bug that some Activity Sytreams properties are not parsed properly (#1129)
- Allow empty avatar for remote users (#1129)
- Percent encode blog FQN for federation interoperability (#1129)
- The same to `preferredUsername` (#1129)

## [[0.7.2]] - 2022-05-11

### Added

- Basque language (#1013)
- Unit tests for ActivityPub (#1021)
- Move to action area after liking/boosting/commenting (#1074)

### Changed

- Bump Rust to nightly 2022-01-26 (#1015)
- Remove "Latest articles" timeline (#1069)
- Change order of timeline tabs (#1069, #1070, #1072)
- Migrate ActivityPub-related crates from activitypub 0.1 to activitystreams 0.7 (#1022)

### Fixed

- Add explanation of sign-up step at sign-up page when email sign-up mode (#1012)
- Add NOT NULL constraint to email_blocklist table fields (#1016)
- Don't fill empty content when switching rich editor (#1017)
- Fix accept header (#1058)
- Render 404 page instead of 500 when data is not found (#1062)
- Reuse reqwest client on broadcasting (#1059)
- Reduce broadcasting HTTP request at once to prevent them being timed out (#1068, #1071)
- Some ActivityPub data (#1021)

## [[0.7.1]] - 2022-01-12

### Added

- Introduce environment variable `MAIL_PORT` (#980)
- Introduce email sign-up feature (#636, #1002)

### Changed

- Some styling improvements (#976, #977, #978)
- Respond with error status code when error (#1002)

### Fiexed

- Fix comment link (#974)
- Fix a bug that prevents posting articles (#975)
- Fix a bug that notification page doesn't show (#981)

## [[0.7.0]] - 2022-01-02

### Added

- Allow `dir` attributes for LtoR text in RtoL document (#860)
- More translation languages (#862)
- Proxy support (#829)
- Riker a actor system library (#870)
- (request-target) and Host header in HTTP Signature (#872)
- Default log levels for RUST_LOG (#885, #886, #919)

### Changed

- Upgrade some dependent crates (#858)
- Use tracing crate (#868)
- Update Rust version to nightly-2021-11-27 (#961)
- Upgrade Tantivy to 0.13.3 and lindera-tantivy to 0.7.1 (#878)
- Run searcher on actor system (#870)
- Extract a function to calculate posts' ap_url and share it with some places (#918)
- Use article title as its slug instead of capitalizing and inserting hyphens (#920)
- Sign GET requests to other instances (#957)

### Fixed

- Percent-encode URI for remote_interact (#866, #857)
- Menu animation not opening on iOS (#876, #897)
- Make actors subscribe to channel once (#913)
- Upsert posts and media instead of trying to insert and fail (#912)
- Update post's ActivityPub id when published by update (#915)
- Calculate media URI properly even when MEDIA_UPLOAD_DIRECTORY configured (#916)
- Prevent duplicated posts in 'all' timeline (#917)
- Draw side line for blockquote on start (#933)
- Fix URIs of posts on Mastodon (#947)
- Place edit link proper position (#956, #963, #964)

## [[0.6.0]] - 2020-12-29

### Added

- Vazir font for better support of languages written in Arabic script (#787)
- Login via LDAP (#826)
- cargo-release (#835)
- Care about weak ETag header for better caching (#840)
- Support for right to left languages in post content (#853)

### Changed

- Bump Docker base images to Buster flavor (#797)
- Upgrade Rocket to 0.4.5 (#800)
- Keep tags as-is (#832)
- Update Docker image for testing (#838)
- Update Dockerfile.dev (#841)

### Fixed

- Recreate search index if its format is outdated (#802)
- Make it possible to switch to rich text editor (#808)
- Fix margins for the mobile devices (#817)
- GPU acceleration for the mobile menu (#818)
- Natural title position for RtoL languages (#825)
- Remove link to unimplemented page (#827)
- Fix displaying not found page when submitting a duplicated blocklist email (#831)

### Security

- Validate spoofing of activity

## [0.5.0] - 2020-06-21

### Added

- Email blocklisting (#718)
- Syntax highlighting (#691)
- Persian localization (#782)
- Switchable tokenizer - enables Japanese full-text search (#776)
- Make database connections configurable by environment variables (#768)

### Changed

- Display likes and boost on post cards (#744)
- Rust 2018 (#726)
- Bump to LLVM to 9.0.0 to fix ARM builds (#737)
- Remove dependency on runtime-fmt (#773)
- Drop the -alpha suffix in release names, it is implied that Plume is not stable yet because of the 0 major version (Plume 1.0.0 will be the first stable release).

### Fixed

- Fix parsing of mentions inside a Markdown code block (be430c6)
- Fix RSS issues (#720)
- Fix Atom feed (#764)
- Fix default theme (#746)
- Fix shown password on remote interact pages (#741)
- Allow unicode hashtags (#757)
- Fix French grammar for for 0 (#760)
- Don't show boosts and likes for "all" and "local" in timelines (#781)
- Fix liking and boosting posts on remote instances (#762)

## [0.4.0] - 2019-12-23

### Added

- Add support for generic timeline (#525)
- Federate user deletion (#551)
- import migrations and don't require diesel_cli for admins (#555)
- Cache local instance (#572)
- Initial RTL support #575 (#577)
- Confirm deletion of blog (#602)
- Make a distinction between moderators and admins (#619)
- Theming (#624)
- Add clap to plume in order to print help and version (#631)
- Add Snapcraft metadata and install/maintenance hooks (#666)
- Add environmental variable to control path of media (#683)
- Add autosaving to the editor (#688)
- CI: Upload artifacts to pull request deploy environment (#539)
- CI: Upload artifact of wasm binary (#571)

### Changed

- Update follow_remote.rs.html grammar (#548)
- Add some feedback when performing some actions (#552)
- Theme update (#553)
- Remove the new index lock tantivy uses (#556)
- Reduce reqwest timeout to 5s (#557)
- Improve notification management (#561)
- Fix occurrences of 'have been' to 'has been' (#578) + Direct follow-up to #578 (#603)
- Store password reset requests in database (#610)
- Use futures and tokio to send activities (#620)
- Don't ignore dotenv errors (#630)
- Replace the input! macro with an Input builder (#646)
- Update default license (#659)
- Paginate the outbox responses. Fixes #669 (#681)
- Use the "classic" editor by default (#697)
- Fix issue #705 (#708)
- Make comments in styleshhets a bit clearer (#545)
- Rewrite circleci config (#558)
- Use openssl instead of sha256sum for build.rs (#568)
- Update dependencies (#574)
- Refactor code to use Shrinkwraprs and diesel-derive-newtype (#598)
- Add enum containing all successful route returns (#614)
- Update dependencies which depended on nix -- fixes arm32 builds (#615)
- Update some documents (#616)
- Update dependencies (#643)
- Make the comment syntax consistent across all CSS (#487)

### Fixed

- Remove r (#535)
- Fix certain improper rendering of forms (#560)
- make hashtags work in profile summary (#562)
- Fix some federation issue (#573)
- Prevent comment form submit button distortion on iOS (#592)
- Update textarea overflow to scroll (#609)
- Fix arm builds (#612)
- Fix theme caching (#647)
- Fix issue #642, frontend not in English if the user language does not exist (#648)
- Don't index drafts (#656)
- Fill entirely user on creation (#657)
- Delete notification on user deletion (#658)
- Order media so that latest added are top (#660)
- Fix logo URL (#664)
- Snap: Ensure cargo-web doesn't erroneously adopt our workspace. (#667)
- Snap: Another fix for building (#668)
- Snap: Fix build for non-Tier-1 Rust platforms (#672)
- Don't split sentences for translations (#677)
- Escape href quotation marks (#678)
- Re-add empty strings in translation (#682)
- Make the search index creation during migration respect SEARCH_INDEX (#689)
- Fix the navigation menu not opening on touch (#690)
- Make search items optional (#693)
- Various snap fixes (#698)
- Fix #637 : Markdown footnotes (#700)
- Fix lettre (#706)
- CI: Fix Crowdin upload (#576)

### Removed

- Remove the Canapi dependency (#540)
- Remove use of Rust in migrations (#704)

## [0.3.0] - 2019-04-19

### Added

- Cover for articles (#299, #387)
- Password reset (#448)
- New editor (#293, #458, #482, #483, #486, #530)
- Search (#324, #375, #445)
- Edit blogs (#460, #494, #497)
- Hashtags in articles (#283, #295)
- API endpoints (#245, #285, #307)
- A bunch of new translations! (#479, #501, #506, #510, #512, #514)

### Changed

- Federation improvements (#216, #217, #357, #364, #399, #443, #446, #455, #502, #519)
- Improved build process (#281, #374, #392, #402, #489, #498, #503, #511, #513, #515, #528)

### Fixes

- UI usability fixes (#370, #386, #401, #417, #418, #444, #452, #480, #516, #518, #522, #532)

## [0.2.0] - 2018-09-12

### Added

- Article publishing, or save as a draft
- Like, or boost an article
- Basic Markdown editor
- Federated commenting system
- User account creation
- Limited federation on other platforms and subscribing to users
- Ability to create multiple blogs

<!-- next-url -->
[Unreleased]: https://github.com/Plume-org/Plume/compare/0.7.2...HEAD
[[0.7.2]]: https://github.com/Plume-org/Plume/compare/0.7.1...0.7.2
[[0.7.1]]: https://github.com/Plume-org/Plume/compare/0.7.0...0.7.1
[[0.7.0]]: https://github.com/Plume-org/Plume/compare/0.6.0...0.7.0
[[0.6.0]]: https://github.com/Plume-org/Plume/compare/0.5.0...0.6.0
[0.5.0]: https://github.com/Plume-org/Plume/compare/0.4.0-alpha-4...0.5.0
[0.4.0]: https://github.com/Plume-org/Plume/compare/0.3.0-alpha-2...0.4.0-alpha-4
[0.3.0]: https://github.com/Plume-org/Plume/compare/0.2.0-alpha-1...0.3.0-alpha-2
[0.2.0]: https://github.com/Plume-org/Plume/releases/tag/0.2.0-alpha-1
