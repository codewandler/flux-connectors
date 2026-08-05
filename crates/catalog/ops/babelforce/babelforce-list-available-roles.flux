op babelforce-list-available-roles -> Any
  description "List available roles"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/users/roles")
  response = http.request(method: "GET", url)
  return response
