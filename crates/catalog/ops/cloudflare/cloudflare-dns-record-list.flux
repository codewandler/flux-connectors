op cloudflare-dns-record-list -> Any
  description "List the DNS records in a zone: each record's id, type, name, content, TTL and whether it is proxied through Cloudflare. Returns Cloudflare's first page only; this connector declares no page or filter parameters (see the connector's header note). A non-2xx response is returned as data, not a failure: the vendor's error message is at `/errors/0/message`, its error code at `/errors/0/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.cloudflare.com/client/v4"
  zone_id = "{zone_id}"
  url = fmt("{base}/zones/{zone_id}/dns_records")
  response = http.request(method: "GET", url)
  return response
