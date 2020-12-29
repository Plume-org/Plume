<h1 align="center">
  <img src="https://raw.githubusercontent.com/Plume-org/Plume/master/assets/icons/trwnh/feather/plumeFeather64.png" alt="Plume's logo">
  Plume
</h1>
<p align="center">
  <a href="https://github.com/Plume-org/Plume/"><img alt="CircleCI" src="https://img.shields.io/circleci/build/gh/Plume-org/Plume.svg"></a>
  <a href="https://codecov.io/gh/Plume-org/Plume"><img src="https://codecov.io/gh/Plume-org/Plume/branch/master/graph/badge.svg" alt="Code coverage"></a>
  <a title="Crowdin" target="_blank" href="https://crowdin.com/project/plume"><img src="https://d322cqt584bo4o.cloudfront.net/plume/localized.svg"></a>
  <a href="https://hub.docker.com/r/plumeorg/plume"><img alt="Docker Pulls" src="https://img.shields.io/docker/pulls/plumeorg/plume.svg"></a>
  <a href="https://liberapay.com/Plume"><img alt="Liberapay patrons" src="https://img.shields.io/liberapay/patrons/Plume.svg"></a>
</p>
<p align="center">
  <a href="https://joinplu.me/">Website</a>
  —
  <a href="https://docs.joinplu.me/">Documentation</a>
  —
  <a href="https://docs.joinplu.me/contribute">Contribute</a>
  —
  <a href="https://joinplu.me/#instances">Instances list</a>
</p>

Plume is a **federated blogging engine**, based on *ActivityPub*. It is written in *Rust*, with the *Rocket* framework, and *Diesel* to interact with the database.
The front-end uses *Ructe* templates, *WASM* and *SCSS*.

## Features

A lot of features are still missing, but what is already here should be quite stable. Current and planned features include:

- **A blog-centric approach**: you can create as much blogs as you want with your account, to keep your different publications separated.
- **Media management**: you can upload pictures to illustrate your articles, but also audio files if you host a podcast, and manage them all from Plume.
- **Federation**: Plume is part of a network of interconnected websites called the Fediverse. Each of these websites (often called *instances*) have their own
rules and thematics, but they can all communicate with each other.
- **Collaborative writing**: invite other people to your blogs, and write articles together.

## Get involved

If you want to have regular news about the project, the best place is probably [our blog](https://fediverse.blog/~/PlumeDev), or our Matrix room: [`#plume-blog:matrix.org`](https://matrix.to/#/#plume-blog:matrix.org).

If you want to contribute more, a good first step is to read [our contribution guides](https://docs.joinplu.me/contribute). We accept all kind of contribution:

- [Back-end or front-end development](https://docs.joinplu.me/contribute/development/)
- [Translations](https://docs.joinplu.me/contribute/translations/)
- [Documentation](https://docs.joinplu.me/contribute/documentation/)
- UI and/or UX design (we don't have a dedicated guide yet, but [we can talk](https://docs.joinplu.me/contribute/discussion/) to see how we can work together!)
- [Taking part in discussions](https://docs.joinplu.me/contribute/discussion/)
- [Financial support](https://docs.joinplu.me/contribute/donations/)

But this list is not exhaustive and if you want to contribute differently you are welcome too!

As we want the various spaces related to the project (GitHub, Matrix, Loomio, etc) to be as safe as possible for everyone, we adopted [a code of conduct](https://docs.joinplu.me/organization/code-of-conduct). Please read it and make sure you accept it before contributing.

## Starting your own instance

We provide various way to install Plume: from source, with pre-built binaries, with Docker or with YunoHost.
For detailed explanations, please refer to [the documentation](https://docs.joinplu.me/installation/).
