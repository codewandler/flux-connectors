op babelforce-call-session-set(id: String, variables: Any) -> Any
  description "Set session variables on a live call. Pass them as the `variables` map; babelforce applies only keys beginning `app.` and silently ignores the rest, and states that rule only in prose"
  risk "medium"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/calls/{id}/session/set")
  content_type = "application/json"
  payload = { variables }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PUT", url)
  return response
