op cloudflare-zone-list -> Any
  description "List the zones (domains) this API token can see, with each zone's id, name, status and assigned Cloudflare name servers. Returns Cloudflare's first page only; this connector declares no page or filter parameters (see the connector's header note). The zone `id` returned here is the value an operator pins as this connection's `zone_id`; every other operation in this connector is already scoped to that one zone and does not take it as an argument. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/errors/0/message`, its error code at `/errors/0/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.cloudflare.com/client/v4"
  url = fmt("{base}/zones")
  response = http.request(method: "GET", url)
  return response
