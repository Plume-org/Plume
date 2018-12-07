# Installing Plume (for development or production)

## Prerequisites

In order to be installed and to work correctly, Plume needs:

- *Git* (to get the code)
- *Curl* (for RustUp, the Rust installer)
- *GCC* and *make*  (to compile C dependencies)
- *PostgreSQL* or *SQlite 3 development files* (for the database)
- *GetText* (to manage translations)
- *Rust* and *Cargo* (to build the code)
- *OpenSSL* and *OpenSSL librairies* (for security)
- *xz* (for gettext-sys compilation)
- *pkg-config* (for openssl-sys compilation)

All the following instructions will need a terminal.

Here are the commands to install PostgreSQL and GetText on various operating systems.
Some of them may need root permissions.

You can also install the project using Docker and docker-compose, please refer
to the `Docker install` section.

On **Debian**:

```bash
apt update

# If you want PostgreSQL
apt install gettext postgresql postgresql-contrib libpq-dev git curl gcc make openssl libssl-dev xz-utils pkg-config

# If you want SQlite
apt install gettext libsqlite3-dev git curl gcc make openssl libssl-dev xz-utils pkg-config

```

On **Fedora**, **CentOS** or **RHEL**:

```bash
# If you want PostgreSQL
dnf install postgresql-server postgresql-contrib libpqxx libpqxx-devel git curl gcc make openssl openssl-devel gettext

# If you want SQLite
dnf install libsq3-devel sqlite3 libsqlite3-dev git curl gcc make openssl openssl-devel gettext
```

On **Gentoo**:

```bash
emerge --sync

# If you want PostgreSQL
emerge -avu dev-db/postgresql dev-vcs/git sys-devel/gettext

# If you want SQlite
emerge -avu dev-db/sqlite dev-vcs/git sys-devel/gettext
```

On **Mac OS X**, with [Homebrew](https://brew.sh/):

```bash
brew update

# For PostgreSQL
brew install postgres gettext git

# For SQlite (already present, so only GetText and Git are needed)
brew install gettext git
```

## Configuring PostgreSQL

You can either run PostgreSQL from the machine that runs Plume, or from another server. We recommend you to use the first setup for development environments, or in production for small instances.

In the first case, just run this command after the PostgreSQL installation, to start it:

```
service postgresql start
```

If you want to have two separate machines, run these commands on the database server once you've installed the dependencies mentioned above on both servers:

```bash
service postgresql start
su - postgres
createuser -d -P plume
createdb -O plume plume
```

## Creating a new user (optional)

This step is recommended if you are in a **production environment**, but it is not necessary.

```bash
adduser plume
su - plume
cd ~
```

Creating a new user will let you use systemd to manage Plume if you want (see the dedicated section below).

## Installing Rust and Cargo

We said that Plume needed Rust and Cargo to work, but we didn't install them at the same time as PostgreSQL and GetText, because there is an universal installation method called RustUp.

You can install it on **GNU/Linux** and **Mac OS X** with:

```bash
curl https://sh.rustup.rs -sSf | sh
```

When asked, choose the *"1) Proceed with installation (default)"* option.

Then run this command to be able to run cargo in the current session:

```bash
export PATH="$PATH:/home/plume/.cargo/bin:/home/plume/.local/bin:/usr/local/sbin"
```

On **Windows**, you'll need, if you don't already have them, to download and install the [Visual C++ 2015 Build Tools](https://www.microsoft.com/en-us/download/details.aspx?id=48159). Then, download the [rustup installer](https://www.rust-lang.org/en-US/install.html) and run it.

## Getting Plume's source code

Plume needs to be compiled from source. To download the code, run:

```bash
git clone https://github.com/Plume-org/Plume.git
cd Plume

# If you want PostgreSQL
export FEATURES=postgres

# If you want SQlite
export FEATURES=sqlite
```

## Configuring Plume

Before starting Plume, you'll need to create a configuration file, called `.env`. Here is a sample of what you should put inside.

```bash
# The address of the database
# (replace USER, PASSWORD, PORT and DATABASE_NAME with your values)
#
# If you are using SQlite, use the path of the database file (`plume.db` for instance)
DATABASE_URL=postgres://USER:PASSWORD@IP:PORT/DATABASE_NAME

# For PostgreSQL: migrations/postgres
# For SQlite: migrations/sqlite
MIGRATION_DIRECTORY=migrations/postgres

# The domain on which your instance will be available
BASE_URL=plu.me

# Secret key used for private cookies and CSRF protection
# You can generate one with `openssl rand -base64 32`
ROCKET_SECRET_KEY=
```

For more information about what you can put in your `.env`, see [the documentation about environment variables](ENV-VARS.md).

## Running migrations

Migrations are scripts used to update the database. They are run by a tool called Diesel, which can be installed with:

```bash
cargo install diesel_cli --no-default-features --features $FEATURES --version '=1.3.0'
```

To run the migrations, you can do:

```bash
diesel migration run
```

Migrations should be run before using Plume or the `plm` CLI tool, and after each update.
When in doubt, run them.

## Running Plume

Then, you'll need to install Plume and the CLI tools to manage your instance.

```
cargo install --no-default-features --features $FEATURES
cargo install --no-default-features --features $FEATURES --path plume-cli
```

After that, you'll need to setup your instance, and the admin's account.

```
plm instance new
plm users new --admin
```

You will also need to initialise search index

```
plm search init -p path/to/plume/workingdir
```

For more information about these commands, and the arguments you can give them, check out [their documentaion](CLI.md).

Finally, you can start Plume with:

```bash
plume
```

We may provide precompiled packages in the future; if you have experience in these fields and want to help, feel free to discuss this in issues and to propose pull-requests!

## Docker install

You can use `docker` and `docker-compose` in order to manage your Plume instance and have it isolated from your host:

```bash
git clone git@github.com:Plume-org/Plume.git
cd Plume
cp docs/docker-compose.sample.yml docker-compose.yml
cp docs/docker.sample.env .env

# Build the containers
docker-compose build

# Launch the database
docker-compose up -d postgres
# Setup the database (create it and run migrations)
docker-compose run --rm plume diesel database setup

# Setup your instance
docker-compose run --rm plume plm instance new
docker-compose run --rm plume plm users new --admin
docker-compose run --rm plume plm search init

# Launch your instance for good
docker-compose up -d
```

Then, you can configure your reverse proxy.

## Configuring Nginx

Here is a sample Nginx configuration for a Plume instance (replace `blog.example.com` with your domain name):

```nginx
server {
    listen 80;
    listen [::]:80;
    server_name blog.example.com;

    location /.well-known/acme-challenge {}
    location / {
        return 301 https://$host$request_uri;
    }
}

server {
    listen 443 ssl http2;
    listen [::]:443 ssl http2;
    server_name blog.example.org;

    access_log  /var/log/nginx/access.log;
    root /home/plume/Plume/ ;

    SSLCertificateFile /etc/letsencrypt/live/blog.example.com/cert.pem
    SSLCertificateKeyFile /etc/letsencrypt/live/blog.example.com/privkey.pem
    SSLCertificateChainFile /etc/letsencrypt/live/blog.example.com/chain.pem

    # for ssl conf: https://cipherli.st/
	ssl_protocols TLSv1.2 TLSv1.3;# Requires nginx >= 1.13.0 else use TLSv1.2
	ssl_prefer_server_ciphers on;
	ssl_dhparam /etc/letsencrypt/ssl-dhparams.pem;# openssl dhparam -out /etc/letsencrypt/ssl-dhparam.pem 4096
	ssl_ciphers ECDHE-RSA-AES256-GCM-SHA512:DHE-RSA-AES256-GCM-SHA512:ECDHE-RSA-AES256-GCM-SHA384:DHE-RSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-SHA384;
	ssl_ecdh_curve secp384r1; # Requires nginx >= 1.1.0
	ssl_session_timeout  10m;
	ssl_session_cache shared:SSL:10m;
	ssl_session_tickets off; # Requires nginx >= 1.5.9
	ssl_stapling on; # Requires nginx >= 1.3.7
	ssl_stapling_verify on; # Requires nginx => 1.3.7
	resolver 9.9.9.9 80.67.169.12 valid=300s;
	resolver_timeout 5s;
	add_header Strict-Transport-Security "max-age=63072000; includeSubDomains; preload";
	add_header X-Frame-Options DENY;
	add_header X-Content-Type-Options nosniff;
	add_header X-XSS-Protection "1; mode=block";
	add_header Content-Security-Policy "default-src 'self' 'unsafe-inline'; frame-ancestors 'self'; frame-src https:";

    location ~*  \.(jpg|jpeg|png|gif|ico|js|pdf)$ {
        add_header Cache-Control "public";
        expires 7d;
    }

    location / {
        proxy_pass http://localhost:7878/;
        proxy_set_header Host $http_host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        client_max_body_size 10m;
    }
}
```

## Configuring Apache

If you prefer Apache, you can use this configuration (here too, replace `blog.example.com` with your domain):

```apache
<VirtualHost *:80>
    ServerName blog.example.com
    Redirect / https://blog.example.com/
</VirtualHost>
SSLStaplingCache "shmcb:logs/stapling-cache(150000)"
<VirtualHost *:443>
   ServerAdmin admin@example.com
   ServerName blog.example.com
<Directory "/home/plume/Plume">
    Header always set Referrer-Policy "strict-origin-when-cross-origin"
    Header always set Strict-Transport-Security "max-age=31536000"
    </Directory>
    SSLEngine on

    # for cipher conf: https://cipherli.st/
    SSLCipherSuite EECDH+AESGCM:EDH+AESGCM:AES256+EECDH:AES256+EDH
    SSLProtocol All -SSLv2 -SSLv3 -TLSv1 -TLSv1.1
    SSLHonorCipherOrder On
    Header always set Strict-Transport-Security "max-age=63072000; includeSubDomains; preload"
    Header always set X-Frame-Options DENY
    Header always set X-Content-Type-Options nosniff
    SSLCompression off
    SSLUseStapling on

    # Requires Apache >= 2.4.11
    SSLSessionTickets Off

    SSLCertificateFile /etc/letsencrypt/live/blog.example.com/cert.pem
    SSLCertificateKeyFile /etc/letsencrypt/live/blog.example.com/privkey.pem
    SSLCertificateChainFile /etc/letsencrypt/live/blog.example.com/chain.pem

    ProxyPreserveHost On
    RequestHeader set X-Forwarded-Proto "https"

    ProxyPass / http://127.0.0.1:7878/
    ProxyPassReverse / http://127.0.0.1:7878/
</VirtualHost>
```

## Configuring Caddyserver

If you prefer [Caddyserver](http://caddyserver.com), you can use this configuration (again, replacing `blog.example.com` with your domain):

```
blog.example.com {
    proxy / localhost:7878 {
        transparent
    }
}
```

## Systemd integration

If you want to manage your Plume instance with systemd, you can use the following unit file (to be saved in `/etc/systemd/system/plume.service`):

```toml
[Unit]
Description=plume

[Service]
Type=simple
User=plume
WorkingDirectory=/home/plume/Plume
ExecStart=/home/plume/.cargo/bin/plume
TimeoutSec=30
Restart=always

[Install]
WantedBy=multi-user.target
```

Now you need to enable all of these services:

```bash
systemctl enable /etc/systemd/system/plume.service
```

Now start the services:

```bash
systemctl start plume.service
```

Check that they are properly running:

```bash
systemctl status plume.service
```

## SysVinit integration

This script can also be useful if you are using SysVinit.

```bash
#!/bin/sh
### BEGIN INIT INFO
# Provides:
# Required-Start:    $remote_fs $syslog
# Required-Stop:     $remote_fs $syslog
# Default-Start:     2 3 4 5
# Default-Stop:      0 1 6
# Short-Description: Start daemon at boot time
# Description:       Federated blogging
# Based on https://raw.githubusercontent.com/fhd/init-script-template/master/template
### END INIT INFO

dir="/home/plume/Plume"
cmd="/home/plume/.cargo/bin/plume"
user="plume"

name=`basename $0`
pid_file="/var/run/$name.pid"
stdout_log="/home/plume/Plume/plume.log"
stderr_log="/home/plume/Plume/plume.err"

get_pid() {
    cat "$pid_file"
}

is_running() {
    [ -f "$pid_file" ] && ps -p `get_pid` > /dev/null 2>&1
}

case "$1" in
    start)
    if is_running; then
        echo "Already started"
    else
        echo "Starting $name"
        cd "$dir"
        if [ -z "$user" ]; then
            sudo $cmd >> "$stdout_log" 2>> "$stderr_log" &
        else
            sudo -u "$user" $cmd >> "$stdout_log" 2>> "$stderr_log" &
        fi
        echo $! > "$pid_file"
        if ! is_running; then
            echo "Unable to start, see $stdout_log and $stderr_log"
            exit 1
        fi
    fi
    ;;
    stop)
    if is_running; then
        echo -n "Stopping $name.."
        kill `get_pid`
        for i in 1 2 3 4 5 6 7 8 9 10
        # for i in `seq 10`
        do
            if ! is_running; then
                break
            fi

            echo -n "."
            sleep 1
        done
        echo

        if is_running; then
            echo "Not stopped; may still be shutting down or shutdown may have failed"
            exit 1
        else
            echo "Stopped"
            if [ -f "$pid_file" ]; then
                rm "$pid_file"
            fi
        fi
    else
        echo "Not running"
    fi
    ;;
    restart)
    $0 stop
    if is_running; then
        echo "Unable to stop, will not attempt to start"
        exit 1
    fi
    $0 start
    ;;
    status)
    if is_running; then
        echo "Running"
    else
        echo "Stopped"
        exit 1
    fi
    ;;
    *)
    echo "Usage: $0 {start|stop|restart|status}"
    exit 1
    ;;
esac

exit 0

```

Now start the services:

```bash
service plume.service start
```


And check:

```bash
service plume.service status
```

## OpenRC integration

This script can also be useful if you are using OpenRC.

```bash
#! /sbin/openrc-run
name="plume"
description="plume : federated blogging"
pidfile=/run/plume
start() {
ebegin "Starting plume"
start-stop-daemon -v --start --exec "/home/plume/.cargo/bin/cargo run" --user "plume" --chdir "/home/plume/Plume" --background --stdout "/var/log/plume.log" --stderr "/var/log/plume.err" --make-pidfile --pidfile "/run/plume" -- "phx.server"
eend $?
}

stop() {
ebegin "Stopping plume"
start-stop-daemon --stop --user "plume" --chdir "/home/plume/Plume" --pidfile "/run/plume"
eend $?
}
```
Now you need to enable all of these services:

```bash
 rc-update add plume
```

Now start the services:

```bash
/etc/init.d/plume start
```



## Caveats:

- Pgbouncer is not supported yet (named transactions are used).
- Rust nightly is a moving target, dependancies can break and sometimes you need to check a few versions to find the one working (run `rustup override set nightly-2018-07-17` in the Plume directory if you have issues during the compilation)

## Acknowledgements

Most of this documentation has been written by *gled-rs*. The systemd unit file, Nginx and Apache configurations have been written by *nonbinaryanargeek*. Some parts (especially the instructions to install native dependencies) are from the [Aardwolf project](https://github.com/Aardwolf-Social/aardwolf). The docker instructions, and files have been added by *Eliot Berriot*.
