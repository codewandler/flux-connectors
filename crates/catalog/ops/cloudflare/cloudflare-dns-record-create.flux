op cloudflare-dns-record-create(type: String, name: String, content: String) -> Any
  description "Create a DNS record in a zone. Cloudflare does not deduplicate: creating the same name/type/content pair twice makes two records, so this is not idempotent. The created record, with its assigned id, is in the response. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/errors/0/message`, its error code at `/errors/0/code` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://api.cloudflare.com/client/v4"
  zone_id = "{zone_id}"
  url = fmt("{base}/zones/{zone_id}/dns_records")
  content_type = "application/json"
  payload = { content, name, type }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
