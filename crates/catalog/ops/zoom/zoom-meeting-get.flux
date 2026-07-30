op zoom-meeting-get(meeting_id: Number) -> Any
  description "Get one meeting — its topic, start time, duration, timezone, join URL and settings. The response also carries `start_url`, which starts the meeting as its host for anyone holding it. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.zoom.us"
  url = fmt("{base}/v2/meetings/{meeting_id}")
  response = http.request(method: "GET", url)
  return response
