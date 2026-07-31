op microsoft_graph-calendar-calendar-get -> Any
  description "Get the signed-in user's own default calendar: its name, colour and sharing/editing permissions. This is the calendar, not its events — use microsoft_graph-calendar-event-get for one of those. Takes no argument: `/me/calendar` always resolves to the token's own default calendar. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://graph.microsoft.com"
  url = fmt("{base}/v1.0/me/calendar")
  response = http.request(method: "GET", url)
  return response
