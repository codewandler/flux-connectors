op babelforce-list-modules -> Any
  description "List application modules"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/applications/modules")
  response = http.request(method: "GET", url)
  return response
