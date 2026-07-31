op postmark-server-list -> Any
  description "List every server on this account. Each entry also carries `ApiTokens`, that server's own live Server Token(s) in plaintext — see that field's own description before handling this response. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/Message`, its error code at `/ErrorCode` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.postmarkapp.com"
  url = fmt("{base}/servers")
  response = http.request(method: "GET", url)
  return response
