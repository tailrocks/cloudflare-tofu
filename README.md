# Tailrocks Cloudflare

OpenTofu-owned Cloudflare DNS for every record in `tailrocks.com`.

Credentials remain in 1Password. `1password.env` contains references only.

```sh
mise trust
mise install
mise run export-dns
mise run check
mise run plan
mise run apply
```

`export-dns` regenerates the complete zone resource and import blocks from Cloudflare. Review every saved plan before applying it. OpenTofu state stays local and uncommitted.
