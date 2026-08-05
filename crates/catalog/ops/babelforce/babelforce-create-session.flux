op babelforce-create-session -> Any
  description "Create a session"
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/sessions")
  response = http.request(method: "POST", url)
  return response
