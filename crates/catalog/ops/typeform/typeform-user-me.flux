op typeform-user-me -> Any
  description "Get the Typeform account this access token authenticates as: its account id, display alias, own account email and interface language. Confirms the token resolves. Takes no parameters. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/description`, its error code at `/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.typeform.com"
  url = fmt("{base}/me")
  response = http.request(method: "GET", url)
  return response
