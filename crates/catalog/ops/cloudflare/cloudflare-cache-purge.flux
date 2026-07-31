op cloudflare-cache-purge -> Any
  description "Purge every cached asset for a zone, immediately, for every visitor worldwide. There is no partial or selective purge in this connector (see its header note) — every call empties the whole zone's edge cache. Repeating it changes nothing further, but each call can spike load on the origin as the cache refills, so it should not be called more often than a real cache-affecting change warrants. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/errors/0/message`, its error code at `/errors/0/code` in the response body."
  risk "high"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.cloudflare.com/client/v4"
  zone_id = "{zone_id}"
  url = fmt("{base}/zones/{zone_id}/purge_cache")
  content_type = "application/json"
  purge_everything = true
  payload = { purge_everything }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
