op zoom-meeting-create(user_id: String, topic: String, start_time: String, duration: Number, waiting_room: Bool) -> Any
  description "Schedule a one-off meeting for a user at a fixed time. Nobody is invited and nobody is notified — the meeting appears on the host's own Zoom schedule and the returned `join_url` is how anyone else learns of it. The response also carries `start_url`, which starts the meeting as its host for anyone holding it. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/code` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.zoom.us"
  url = fmt("{base}/v2/users/{user_id}/meetings")
  content_type = "application/json"
  type = 2
  payload = { duration, settings: { waiting_room }, start_time, topic, type }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
