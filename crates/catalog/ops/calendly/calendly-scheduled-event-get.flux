op calendly-scheduled-event-get(uuid: String) -> Any
  description "Get one scheduled event by id. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/title` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.calendly.com"
  url = fmt("{base}/scheduled_events/{uuid}")
  response = http.request(method: "GET", url)
  return response
