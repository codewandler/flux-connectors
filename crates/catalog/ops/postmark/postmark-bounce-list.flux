op postmark-bounce-list(count: Number, offset: Number) -> Any
  description "List bounces recorded on this server, most recent first. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/Message`, its error code at `/ErrorCode` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.postmarkapp.com"
  url = fmt("{base}/bounces")
  response = http.request(method: "GET", query: { count, offset }, url)
  return response
