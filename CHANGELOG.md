# Changelog

<!-- next-header -->

## [Unreleased] - ReleaseDate

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

- Validate spoofing of Create activity

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
[Unreleased]: https://github.com/Plume-org/Plume/compare/0.6.0...HEAD
[[0.6.0]]: https://github.com/Plume-org/Plume/compare/0.5.0...0.6.0
[0.5.0]: https://github.com/Plume-org/Plume/compare/0.4.0-alpha-4...0.5.0
[0.4.0]: https://github.com/Plume-org/Plume/compare/0.3.0-alpha-2...0.4.0-alpha-4
[0.3.0]: https://github.com/Plume-org/Plume/compare/0.2.0-alpha-1...0.3.0-alpha-2
[0.2.0]: https://github.com/Plume-org/Plume/releases/tag/0.2.0-alpha-1
