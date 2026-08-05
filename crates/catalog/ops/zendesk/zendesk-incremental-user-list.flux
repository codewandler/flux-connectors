op zendesk-incremental-user-list(start_time: Number, per_page: Number) -> Any
  description "Incrementally export users updated at or after a required Unix start time with an optional integer page size"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/incremental/users")
  response = http.request(method: "GET", query: { per_page, start_time }, url)
  return response
