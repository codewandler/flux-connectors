op postmark-bounce-get(bounce_id: Number) -> Any
  description "Get one recorded bounce by id. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/Message`, its error code at `/ErrorCode` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.postmarkapp.com"
  url = fmt("{base}/bounces/{bounce_id}")
  response = http.request(method: "GET", url)
  return response
