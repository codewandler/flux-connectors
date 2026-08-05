op google-calendar-event-get(calendar_id: String, event_id: String) -> Any
  description "Get one calendar event: its summary, start and end, organizer, attendee list and status. Needs the `calendar.events.readonly` scope (or `calendar.readonly`). A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/status` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://www.googleapis.com"
  url = fmt("{base}/calendar/v3/calendars/{calendar_id}/events/{event_id}")
  response = http.request(method: "GET", url)
  return response
