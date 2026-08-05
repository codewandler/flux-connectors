op front-verify(limit: Number) -> Any
  description "List one conversation, confirming the token resolves and carries at least read access. Takes no parameters other than the page bound. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/_error/message`, its error code at `/_error/status` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api2.frontapp.com"
  url = fmt("{base}/conversations")
  response = http.request(method: "GET", query: { limit }, url)
  return response
