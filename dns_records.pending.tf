# Pending record: skills.tailrocks.com serves the tailrocks-skills documentation
# from GitHub Pages, matching the termrock.tailrocks.com record.
#
# This file exists only until the record is created. Afterwards:
#
#   mise run export-dns   # rewrites dns_records.generated.tf with the record's
#                         # Cloudflare id and adds its import block
#   tofu state rm cloudflare_dns_record.cname_skills_tailrocks_com_pending
#   rm dns_records.pending.tf
#   mise run check && mise run plan   # plan must show no changes
#
# prevent_destroy is deliberately omitted so the state entry can be released
# once the generated resource takes ownership.
resource "cloudflare_dns_record" "cname_skills_tailrocks_com_pending" {
  zone_id = local.zone_id
  name    = "skills.tailrocks.com"
  type    = "CNAME"
  ttl     = 1
  content = "tailrocks.github.io"
  proxied = false
  settings = {
    "flatten_cname" : false
  }
}
