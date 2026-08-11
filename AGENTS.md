# Tailrocks Cloudflare contributor rules

- Use `tofu`, never `terraform`.
- Never commit state, plan files, `.tfvars`, `.env`, API credentials, or private keys.
- Resolve Cloudflare credentials through `op run --env-file 1password.env`.
- Run `mise run check` before every commit.
- Review `tofu plan` before apply. Apply only operator-requested DNS changes.
- Commit with DCO signoff and push immediately.
