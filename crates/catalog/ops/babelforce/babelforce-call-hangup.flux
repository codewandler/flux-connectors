op babelforce-call-hangup(id: String) -> Any
  description "Hang up a call"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/calls/{id}/hangup")
  response = http.request(method: "POST", url)
  return response
