op babelforce-cancel-call(id: String) -> Any
  description "Cancel a call"
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/calls/{id}/cancel")
  response = http.request(method: "POST", url)
  return response
