op box-user-me -> Any
  description "Get the user this access token authenticates as, confirming the token resolves and naming the account it belongs to. Takes no parameters. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.box.com"
  url = fmt("{base}/2.0/users/me")
  response = http.request(method: "GET", url)
  return response
