op babelforce-testing(actions: Any, input: Any) -> Any
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/action/test")
  content_type = "application/json"
  payload = { actions, input }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
