// This is the CI config for Plume.
// It uses a Drone CI instance, on https://ci.joinplu.me

// First of all, we define a few useful constants

// This Docker image contains everything we need to build Plume.
// Its Dockerfile can be found at https://git.joinplu.me/plume/buildenv
local plumeEnv = "plumeorg/plume-buildenv:v0.0.9";

// A utility function to generate a new pipeline
local basePipeline(name, steps) = {
    kind: "pipeline",
    name: name,
    type: "docker",
    steps: steps
};

// A pipeline step that restores the cache.
// The cache contains all the cargo build files.
// Thus, we don't have to download and compile all of our dependencies for each
// commit.
// This cache is only "deleted" when the contents of Cargo.lock changes.
//
// We use this plugin for caching: https://github.com/meltwater/drone-cache/
//
// Potential TODO: use one cache per pipeline, as we used to do when we were
// using CircleCI.
local restoreCache = {
    name: "restore-cache",
    image: "meltwater/drone-cache:dev",
    pull: true,
    settings: {
        backend: "filesystem",
        restore: true,
        cache_key: 'v0-{{ checksum "Cargo.lock" }}-{{ .Commit.Branch }}',
        archive_format: "gzip",
        mount: [ "~/.cargo/", "./target" ]
    },
    volumes: { name: "cache", path: "/tmp/cache" }
};

// And a step that saves the cache.
local saveCache = {
    name: "save-cache",
    image: "meltwater/drone-cache:dev",
    pull: true,
    settings: {
        backend: "filesystem",
        rebuild: true,
        cache_key: 'v0-{{ checksum "Cargo.lock" }}-{{ .Commit.Branch }}',
        archive_format: "gzip",
        mount: [ "~/.cargo/", "./target" ]
    },
    volumes: { name: "cache", path: "/tmp/cache" }
};

// Finally, the Docker volume to store the cache
local cacheVolume = {
    name: "cache",
    host: {
        path: "/var/lib/cache"
    }
};

// This step starts a PostgreSQL database if the db parameter is "postgres",
// otherwise it does nothing.
local startDb(db) = if db == "postgres" then {
    name: "start-db",
    image: "postgres:9.6-alpine",
    detach: true,
    environment: {
        POSTGRES_USER: "postgres",
        POSTGRES_DB: "plume"
    }
};

// Here starts the actual list of pipelines!

// First one: a pipeline that runs cargo fmt, and that fails if the style of
// the code is not standard.
local CargoFmt() = basePipeline(
    "cargo-fmt",
    [
        restoreCache,
        {
            name: "cargo-fmt",
            image: plumeEnv,
            commands: [ "cargo fmt --all -- --check" ]
        },
        saveCache,
    ]
);

local Clippy(db) = basePipeline(
    "clippy-" + db,
    [
        restoreCache,
        {
            local cmd(pkg, features=true) = if features then
                "cargo clippy --no-default-features --features " + db
                + "--release -p " + pkg + " -- -D warnings"
            else
                "cargo clippy --no-default-features --release -p "
                + pkg + " -- -D warnings",
            name: "clippy",
            image: plumeEnv,
            commands: [
                cmd("plume"), cmd("plume-cli"), cmd("plume-front", false)
            ],
        },
        saveCache,
    ]
);

// TODO

local Unit(db) = {};
local Integration(db) = {};
local Release(db) = {};
local PushTranslations() = {};

// And finally, the list of all our pipelines:
[
    CargoFmt(),
    Clippy("postgres"),
    Clippy("sqlite"),
    Unit("postgres"),
    Unit("sqlite"),
    Integration("postgres"),
    Integration("sqlite"),
    Release("postgres"),
    Release("sqlite"),
    PushTranslations()
]
