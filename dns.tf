resource "cloudflare_dns_record" "cname_termrock_tailrocks_com" {
  zone_id = local.zone_id
  name    = "termrock.tailrocks.com"
  type    = "CNAME"
  ttl     = 1
  content = "tailrocks.github.io"
  proxied = false

  settings = {
    flatten_cname = false
  }

  lifecycle {
    prevent_destroy = true
  }
}
