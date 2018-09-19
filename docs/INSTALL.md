# Installing Plume (for development or production)

## Prerequisites

In order to be installed and to work correctly, Plume needs:

- *Git* (to get the code)
- *Curl* (for RustUp, the Rust installer)
- *GCC* and *make*  (to compile C dependencies)
- *PostgreSQL* (for the database)
- *GetText* (to manage translations)
- *Rust* and *Cargo* (to build the code)
- *OpenSSL* and *OpenSSL librairies* (for security)

All the following instructions will need a terminal.

Here are the commands to install PostgreSQL and GetText on various operating systems.
Some of them may need root permissions.

You can also install the project using Docker and docker-compose, please refer
to the `Docker install` section.

On **Debian**:

```bash
apt update
apt install gettext postgresql postgresql-contrib libpq-dev git curl gcc make openssl libssl-dev
```

On **Fedora**, **CentOS** or **RHEL**:

```bash
dnf install postgresql-server postgresql-contrib mariadb-devel libsq3-devel libpqxx libpqxx-devel git curl gcc make openssl openssl-devel gettext
```

On **Gentoo**:

```bash
emerge --sync
emerge -av postgresql eselect-postgresql gettext && emerge --ask dev-vcs/git
```

On **Mac OS X**, with [Homebrew](https://brew.sh/):

```bash
brew update
brew install postgres gettext git
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

## Getting and compiling the Plume source code

Plume needs to be compiled from source.

```bash
git clone https://github.com/Plume-org/Plume.git
cd Plume

# This may take some time as RustUp will download all
# the required Rust components, and Cargo will download
# and compile all dependencies.
cargo build
```

We may provide precompiled packages and Docker images in the future; if you have experience in these fields and want to help, feel free to discuss this in issues and to propose pull-requests!

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


## Running migrations

Migrations are scripts used to update the database. They are run by a tool called Diesel, which can be installed with:

```bash
cargo install diesel_cli --no-default-features --features postgres --version '=1.2.0'
```

Plume should normally run migrations on your behalf as needed, but if you want to run them manually, use the following command:

```bash
diesel migration run --database-url postgres://USER:PASSWORD@IP:PORT/DATABASE_NAME
```

This command may be useful if you decided to use a separate database server.

## Starting Plume

When you launch Plume for the first time, it will ask you a few questions to setup your instance before it actually launches. To start it, run these commands.

```
# Optional, only do it if the database URL is not
# postgres://plume:plume@localhost/plume
export DB_URL=postgres://plume:PASSWORD@DBSERVERIP:DBPORT/DATABASE_NAME

# Create the media directory, where uploads will be stored
mkdir media

# Actually start Plume
cargo run
```

## Docker install

You can use `docker` and `docker-compose` in order to manage your Plume instance and
have it isolated from your host:

```
git clone git@github.com:Plume-org/Plume.git
cd Plume
cp docs/docker-compose.sample.yml docker-compose.yml
cp docs/docker.sample.env .env
# build the containers
docker-compose build
# launch the database
docker-compose up -d postgres
# run the migrations
docker-compose run --rm plume diesel migration run
# run interactive setup
docker-compose run --rm plume bash
cargo run
# copy the env file and paste it in your host .env file
cat .env
# leave the container
exit
# launch your instance for good
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
	add_header Content-Security-Policy "default-src 'self'; frame-ancestors 'self'; frame-src https:";

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

<VirtualHost *:443>
   ServerAdmin admin@example.com
   ServerName blog.example.com
<Directory "/home/plume/Plume">
    Header always set Referrer-Policy "strict-origin-when-cross-origin"
    Header always set Strict-Transport-Security "max-age=31536000"
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
    SSLStaplingCache "shmcb:logs/stapling-cache(150000)"

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

## Systemd integration

If you want to manage your Plume instance with systemd, you can use the following unit file (to be saved in `/etc/systemd/system/plume.service`):

```toml
[Unit]
Description=plume

[Service]
Type=simple
User=plume
WorkingDirectory=/home/plume/Plume
ExecStart=/home/plume/.cargo/bin/cargo run
TimeoutSec=30
Restart=always

[Install]
WantedBy=multi-user.target
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
cmd="/home/plume/.cargo/bin/cargo run"
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

## Caveats:

- Pgbouncer is not supported yet (named transactions are used).
- Rust nightly is a moving target, dependancies can break and sometimes you need to check a few versions to find the one working (run `rustup override set nightly-2018-05-15` or `rustup override set nightly-2018-05-31` in the Plume directory if you have issues during the compilation)
- Rust nightly 2018-06-28 is known to be failing to compile diesel 1.3.2

## Acknowledgements

Most of this documentation has been written by *gled-rs*. The systemd unit file, Nginx and Apache configurations have been written by *nonbinaryanargeek*. Some parts (especially the instructions to install native dependencies) are from the [Aardwolf project](https://github.com/Aardwolf-Social/aardwolf).
