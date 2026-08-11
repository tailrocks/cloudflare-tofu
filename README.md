# Tailrocks Cloudflare

OpenTofu-owned Cloudflare DNS for `tailrocks.com`.

Credentials remain in 1Password. `1password.env` contains references only.

```sh
mise trust
mise install
mise run check
mise run plan
mise run apply
```

Review every saved plan before applying it. OpenTofu state stays local and uncommitted.
