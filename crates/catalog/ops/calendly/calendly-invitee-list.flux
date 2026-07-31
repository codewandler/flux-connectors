op calendly-invitee-list(uuid: String, status: String, count: Number) -> Any
  description "List a scheduled event's invitees. Each invitee is personal data about a named third party — their name, email and any answers they gave to the event type's custom questions. Read it only for what the calling flow needs and do not persist or repeat it beyond that. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/title` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.calendly.com"
  url = fmt("{base}/scheduled_events/{uuid}/invitees")
  sep = "?"
  when status
    url = fmt("{url}{sep}status={status}")
    sep = "&"
  when count
    url = fmt("{url}{sep}count={count}")
  response = http.request(method: "GET", url)
  return response
