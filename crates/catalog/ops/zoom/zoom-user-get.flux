op zoom-user-get(user_id: String) -> Any
  description "Get one user — their id, email, name, timezone, licence type and personal meeting id. This is how `me` is resolved to the id a meeting is created under. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  $base = "https://api.zoom.us"
  $url = fmt("{base}/v2/users/{user_id}")
  $response = http.request({ method: "GET", url: $url })
  return $response
