op babelforce-list-secret-prefixes -> Any
  description "Retrieves a list of secret prefixes"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/configurations/secrets")
  response = http.request(method: "GET", url)
  return response
