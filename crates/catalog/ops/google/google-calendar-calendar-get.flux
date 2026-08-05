op google-calendar-calendar-get(calendar_id: String) -> Any
  description "Get one calendar's own metadata — its summary, description, location and time zone. This is the calendar, not its events: use `google-calendar-event-get` for one of those. Needs the `calendar.readonly` scope. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/status` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://www.googleapis.com"
  url = fmt("{base}/calendar/v3/calendars/{calendar_id}")
  response = http.request(method: "GET", url)
  return response
