op microsoft_graph-calendar-event-create(subject: String, start_date_time: String, start_time_zone: String, end_date_time: String, end_time_zone: String) -> Any
  description "Create a single-instance event on the signed-in user's default calendar. No attendees can be declared yet (C-56), so nobody is invited and nobody is notified — the event appears only on the calendar it was created on. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/code` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://graph.microsoft.com"
  url = fmt("{base}/v1.0/me/events")
  content_type = "application/json"
  payload = { end: { dateTime: end_date_time, timeZone: end_time_zone }, start: { dateTime: start_date_time, timeZone: start_time_zone }, subject }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
