op zendesk-incremental-ticket-event-list(start_time: Number) -> Any
  description "Incrementally export ticket audit events at or after a required Unix start time"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/incremental/ticket_events")
  response = http.request(method: "GET", query: { start_time }, url)
  return response
