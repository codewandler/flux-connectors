op babelforce-list-secret-keys(prefix: String) -> Any
  description "Retrieves a list of secret keys"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/configurations/secrets/{prefix}")
  response = http.request(method: "GET", url)
  return response
