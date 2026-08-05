op babelforce-clone-trigger(id: String) -> Any
  description "Clone a trigger"
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/triggers/{id}/clone")
  response = http.request(method: "POST", url)
  return response
