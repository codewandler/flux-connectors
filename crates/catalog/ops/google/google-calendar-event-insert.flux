op google-calendar-event-insert(calendar_id: String, summary: String, start_time: String, end_time: String) -> Any
  description "Create a timed event on a calendar. No attendees can be declared yet, so nobody is invited and no notification is sent; invite people in Calendar afterwards. Needs the `calendar.events` scope. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/status` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  $base = "https://www.googleapis.com"
  $url = fmt("{base}/calendar/v3/calendars/{calendar_id}/events")
  $content_type = "application/json"
  $payload = { end: { dateTime: $end_time }, start: { dateTime: $start_time }, summary: $summary }
  $response = http.request({ body: $payload, headers: { "content-type": $content_type }, method: "POST", url: $url })
  return $response
