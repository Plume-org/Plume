# Installing Plume (for development or production)

## Prerequisites

In order to be installed and to work correctly, Plume needs:

- Git
- PostgreSQL
- GetText
- Rust and Cargo

All the following instructions will need a terminal.

Here are the commands to install PostgreSQL and GetText on various operating systems.
Some of them may need root permissions.

On **Debian**:

```bash
apt update
apt install gettext postgresql postgresql-contrib libpq-dev git
```

On **Fedora**, **CentOS** or **RHEL**:

```bash
dnf install postgresql-server postgresql-contrib mariadb-devel libsq3-devel libpqxx libpqxx-devel
# TODO: GetText + Git install
```

On **Gentoo**:

```bash
emerge --sync
emerge -av postgresql eselect-postgresql
# TODO: GetText + Git install
```

On **Mac OS X**, with [Homebrew](https://brew.sh/):

```bash
brew update
brew install postgres
# TODO: GetText + Git install
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

We said that Plume needed Rust and Cargo to work, but we didn't installed them at the same time as PostgreSQL and GetText, because there is an universal installation method called RustUp.

You can install it on **GNU/Linux** and **Mac OS X** with:

```bash
curl https://sh.rustup.rs -sSf | sh
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

We may provide precompiled packages and Docker images in the future (if you have experience in these fields and want to help, you're welcome).

## Configuring PostgreSQL

You can either run PostgreSQL from the machine that runs Plume, or from another server. We recommend you to use the first setup for development environments, or in production for small instances.

In the first case, just run this command after the PostgreSQL installation, to start it:

```
service postgresql start
```

If you want to have two separate machines, run these commands on the database server after you installed the dependencies mentionned above on both servers:

```bash
service postgresql start
su - postgres
createuser -d -P plume
createdb -O plume plume
```

```bash
```

## Running migrations

Migrations are scripts to update the database. They are run by a tool called Diesel, which can be installed with:

```bash
cargo install diesel_cli --no-default-features --features postgres --version '=1.2.0'
```

Plume should normally run migrations for you when needed, but if you want to run them manually, the command is:

```bash
diesel migration run --database-url postgres://USER:PASSWORD@IP:PORT/plume
```

This command may be useful if you decided to use a separate database server.

## Starting Plume

When you launch Plume for the first time, it will ask you a few questions to setup your instance before it actually launches. To start it, run these commands.

```
# Optional, only do it if the database URL is not
# postgres://plume:plume@localhost/plume
export DB_URL=postgres://plume:PASSWORD@DBSERVERIP:DBPORT/plume

cargo run
```

## Configuring Nginx

Here is a sample Nginx configuration for a Plume instance (replace `blog.example.com` with your domain name):

```nginx
server {
    listen 80;
    server_name blog.example.com;

    location /.well-known/acme-challenge {}
    location / {
        return 301 https://$host$request_uri;
    }
}

server {
    server_name blog.example.org;
    access_log  /var/log/nginx/access.log;

    listen [::]:443 ssl; # managed by Certbot
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
	add_header Content-Security-Policy "default-src 'self';";
	add_header Content-Security-Policy "frame-ancestors 'self'";

    location ~*  \.(jpg|jpeg|png|gif|ico|js|pdf)$ {
        add_header Cache-Control "public";
        expires 7d;
    }

    location / {
        proxy_pass http://localhost:8000/;
        proxy_set_header Host $http_host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        client_max_body_size 10m;
    }
}
```

## Configuring Apache

If you prefer Apache, you can use this configuration (here too replace `blog.example.com` with your domain):

```apache
<VirtualHost *:80>
    ServerName blog.example.com
    Redirect / https://blog.example.com/
</VirtualHost>

<VirtualHost *:443>
   ServerAdmin admin@example.com
   ServerName blog.example.com

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

If you want to manage your Plume instance with systemd, you can use the following unit file (to be saved in `/lib/systemd/system/plume.service`):

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

## Caveats:

- Pgbouncer is not yet supported (named transactions are used).
- Rust nightly is a moving target, dependancies can break and sometimes you need to check a few versions to find the one working (run `rustup override set nightly-2018-05-15` or `rustup override set nightly-2018-05-31` in the Plume directory if you have issues during the compilation)
- Rust nightly 2018-06-28 is known to be failing to compile diesel 1.3.2

## Acknowledgements

Most of this documentation have been written by *gled-rs*. The systemd unit file, Nginx and Apache configurations have been written by *nonbinaryanargeek*. Some parts (especially the instructions to install native dependencies) are from the [Aardwolf project](https://github.com/Aardwolf-Social/aardwolf).
