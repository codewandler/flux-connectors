op babelforce-call-get(id: String) -> Any
  description "Get a call"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/calls/{id}")
  response = http.request(method: "GET", url)
  return response
