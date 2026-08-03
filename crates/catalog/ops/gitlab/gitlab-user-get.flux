op gitlab-user-get -> Any
  description "Get the currently authenticated user. Takes no parameters; used as the verify read to prove a token resolves"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "{origin}/api/v4"
  url = fmt("{base}/user")
  response = http.request(method: "GET", url)
  return response
