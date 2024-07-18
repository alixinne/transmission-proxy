# [transmission-proxy](https://github.com/alixinne/transmission-proxy)

[![Build Status](https://github.com/alixinne/efiboot-rs/actions/workflows/build.yml/badge.svg)](https://github.com/alixinne/efiboot-rs/actions/workflows/build.yml)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Built with cargo-make](https://sagiegurari.github.io/cargo-make/assets/badges/cargo-make.svg)](https://sagiegurari.github.io/cargo-make)

transmission-proxy is an OAuth2 / Basic auth proxy for the
[Transmission](https://transmissionbt.com/) BitTorrent client, providing
fine-grained ACL and per-user download folders for multi-user use cases.

## Table of contents

<!-- vim-markdown-toc GFM -->

* [Configuration](#configuration)
* [Running](#running)
* [Author](#author)

<!-- vim-markdown-toc -->

## Configuration

transmission-proxy requires a `transmission-proxy.yaml` file to be provided as
input to configure:

- ACL rules: which identities are allowed what actions
- Providers: ways of authenticating users for mapping them to identities

Here's an example `transmission-proxy.yaml` which provides:

- Admin access to admin@gmail.com (or admin basic auth user)
- Read-only access to readonly@gmail.com (or readonly basic auth user)
- Blocks access to anonymous users

For this configuration to be functional, you'll need to:

- Set the bcrypt hash of the password for basic auth users
- Generate a `client_id`/`client_secret` for a Google client application to
  enable Google OAuth from the console

```yaml
# transmission-proxy.yaml
acl:
  rules:
    - identities:
        - provider: basic
          name: admin
        - provider: oauth2
          oauth2: google
          name: admin@gmail.com
    - identities:
        - provider: basic
          name: readonly
        - provider: oauth2
          oauth2: google
          name: readonly@gmail.com
      allowed_methods:
        - torrent-get
        - session-get
        - session-stats
        - free-space
    - deny: true

providers:
  basic:
    enabled: true
    visible: false
    users:
      - username: admin
        password: "*bcrypt hash of the password*"
      - username: readonly
        password: "*bcrypt hash of the password*"
  oauth2:
    - auth_url: https://accounts.google.com/o/oauth2/v2/auth
      client_id: ...
      client_secret: ...
      email_path: $.email
      name: google
      token_url: https://www.googleapis.com/oauth2/v3/token
      userinfo_url: https://www.googleapis.com/oauth2/v3/userinfo
```

## Running

You can run the proxy from its Docker image:

```
docker run -v transmission-proxy.yaml:/transmission-proxy.yaml:ro ghcr.io/alixinne/transmission-proxy
```

Check the available options with `--help` to configure integration with your
existing transmission daemon.

## Author

Alixinne <alixinne@pm.me>
