// This is the CI config for Plume.
// It uses a Drone CI instance, on https://ci.joinplu.me

// First of all, we define a few useful constants

// This Docker image contains everything we need to build Plume.
// Its Dockerfile can be found at https://git.joinplu.me/plume/buildenv
local plumeEnv = "plumeorg/plume-buildenv:v0.2.0";

// Common cache config
local cacheConfig(name, extra) = {
    name: name,
    image: "meltwater/drone-cache:dev",
    pull: true,
    environment: {
      AWS_ACCESS_KEY_ID: { from_secret: 'minio_key' },
      AWS_SECRET_ACCESS_KEY: { from_secret: 'minio_secret' },
    },
    settings: extra + {
        cache_key: 'v0-{{ checksum "Cargo.lock" }}-{{ .Commit.Branch }}',
        archive_format: "gzip",
        mount: [ "~/.cargo/", "./target" ],
        bucket: 'cache',
        path_style: true,
        endpoints: "127.0.0.1:9000",
        region: 'us-east-1',
        debug: true,
    },
    volumes: [ { name: "cache", path: "/tmp/cache" } ]
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
local restoreCache = cacheConfig("restore-cache", { restore: true });
// And a step that saves the cache.
local saveCache = cacheConfig("save-cache", { rebuild: true });

// This step starts a PostgreSQL database if the db parameter is "postgres",
// otherwise it does nothing.
local startDb(db) = if db == "postgres" then {
    name: "start-db",
    image: "postgres:12.3-alpine",
    detach: true,
    environment: {
        POSTGRES_USER: "plume",
        POSTGRES_DB: "plume",
        POSTGRES_PASSWORD: "password",
    }
} else {};

// A utility function to generate a new pipeline
local basePipeline(name, steps) = {
    kind: "pipeline",
    name: name,
    type: "docker",
    environment: {
        RUST_TEST_THREADS: '1',
    },
    steps: steps
};

// And this function creates a pipeline with caching
local cachedPipeline(name, commands) = basePipeline(
    name,
    [
        restoreCache,
        {
            name: name,
            image: plumeEnv,
            commands: commands,
        },
        saveCache
    ]
);

// This function creates a step to upload artifacts to Minio
local upload(name, source) = {
    name: name,
    image: 'plugins/s3',
    settings: {
        bucket: 'artifacts',
        source: source,
        target: '/${DRONE_BUILD_NUMBER}',
        path_style: true,
        endpoint: 'http://127.0.0.1:9000',
        access_key: { from_secret: 'minio_key' },
        secret_key: { from_secret: 'minio_secret' },
    },
};


// Here starts the actual list of pipelines!

// PIPELINE 1: a pipeline that runs cargo fmt, and that fails if the style of
// the code is not standard.
local CargoFmt() = cachedPipeline(
    "cargo-fmt",
    [ "cargo fmt --all -- --check" ]
);

// PIPELINE 2: runs clippy, a tool that helps
// you writing idiomatic Rust.

// Helper function:
local cmd(db, pkg, features=true) = if features then
    "cargo clippy --no-default-features --features " + db + " --release -p "
    + pkg + " -- -D warnings"
else
    "cargo clippy --no-default-features --release -p "
    + pkg + " -- -D warnings";

// The actual pipeline:
local Clippy(db) = cachedPipeline(
    "clippy-" + db,
    [
        cmd(db, "plume"),
        cmd(db, "plume-cli"),
        cmd(db, "plume-front", false)
    ]
);

// PIPELINE 3: runs unit tests
local Unit(db) = cachedPipeline(
    "unit-" + db,
    [
        "cargo test --all --exclude plume-front --exclude plume-macro"
        + " --no-run --no-default-features --features=" + db
    ]
);

// PIPELINE 4: runs integration tests
// It installs a local instance an run integration test with Python scripts
// that use Selenium (located in scripts/browser_test).
local Integration(db) = basePipeline(
    "integration-" + db,
    [
        restoreCache,
        startDb(db),
        {
            name: 'selenium',
            image: 'elgalu/selenium:latest',
            detach: true,
        },
        {
            name: "integration",
            image: plumeEnv,
            environment: {
                BROWSER: "firefox",
                DATABASE_URL: if db == "postgres" then "postgres://plume:password@start-db/plume" else "plume.db",
            },
            commands: [
                // Install the front-end
                "cargo web deploy -p plume-front",
                // Install the server
                'cargo install --debug --no-default-features --features="'
                + db + '",test --force --path .',
                // Install plm
                'cargo install --debug --no-default-features --features="'
                + db + '" --force --path plume-cli',
                // Run the tests
                "./script/run_browser_test.sh"
            ],
        },
        saveCache,
    ]
);

// PIPELINE 5: make a release build and save artifacts
//
// It should also deploy the SQlite build to a test instance
// located at https://pr-XXX.joinplu.me (but this system is not very
// stable, and often breaks).
local Release(db) = basePipeline(
    "release-" + db,
    [
        restoreCache,
        {
            name: 'release-' + db,
            image: plumeEnv,
            commands: [
                "cargo web deploy -p plume-front --release",
                "cargo build --release --no-default-features --features=" + db + " -p plume",
                "cargo build --release --no-default-features --features=" + db + " -p plume-cli",
                "./script/generate_artifact.sh",
            ] + if db == "sqlite" then
            [ "./script/upload_test_environment.sh" ] else
            []
        },
        upload('artifacts-' + db, '*.tar.gz'),
        saveCache,
    ]

);

// PIPELINE 6: upload the new PO templates (.pot) to Crowdin
//
// TODO: run only on master
local PushTranslations() = basePipeline(
    "push-translations",
    [
        {
            name: "push-translations",
            image: plumeEnv,
            commands: [
                "cargo build",
                "crowdin upload -b master"
            ]
        }
    ]
);

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
