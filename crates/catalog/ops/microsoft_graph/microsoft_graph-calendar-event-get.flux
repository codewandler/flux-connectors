op microsoft_graph-calendar-event-get(event_id: String) -> Any
  description "Get one calendar event: its subject, start and end, organizer, attendee list and cancellation state. Returned in UTC unless a `Prefer: outlook.timezone` header is sent, which this connector does not do. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://graph.microsoft.com"
  url = fmt("{base}/v1.0/me/events/{event_id}")
  response = http.request(method: "GET", url)
  return response
