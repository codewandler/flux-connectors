op calendly-scheduled-event-list(user: String, status: String, count: Number) -> Any
  description "List a user's scheduled events (past and upcoming bookings). Takes the user's own URI, not a bare id"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.calendly.com"
  url = fmt("{base}/scheduled_events")
  response = http.request(method: "GET", query: { count, status, user }, url)
  return response
