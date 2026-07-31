op calendly-user-me -> Any
  description "Get the authenticated user: their own resource URI, name, scheduling slug, account email and current organization. The URI in the response is the value later operations expect as their `user` query parameter"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.calendly.com"
  url = fmt("{base}/users/me")
  response = http.request(method: "GET", url)
  return response
