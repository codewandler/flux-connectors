op calendly-event-type-list(user: String, count: Number) -> Any
  description "List event types owned by a user — the bookable meeting templates (name, duration, scheduling link). Takes the user's own URI, not a bare id"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.calendly.com"
  url = fmt("{base}/event_types?user={user}")
  sep = "&"
  when count
    url = fmt("{url}{sep}count={count}")
  response = http.request(method: "GET", url)
  return response
