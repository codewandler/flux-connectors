op zendesk-incremental-ticket-list(start_time: Number) -> Any
  description "Incrementally export tickets updated at or after a required Unix start time"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/incremental/tickets")
  response = http.request(method: "GET", query: { start_time }, url)
  return response
