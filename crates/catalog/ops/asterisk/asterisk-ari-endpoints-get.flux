op asterisk-ari-endpoints-get(tech: String, resource: String) -> Any
  description "Details for an endpoint."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/endpoints/{tech}/{resource}")
  response = http.request(method: "GET", url)
  return response
