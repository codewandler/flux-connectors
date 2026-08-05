op babelforce-list-trigger-conditions(id: String) -> Any
  description "List a trigger's conditions"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/triggers/{id}/conditions")
  response = http.request(method: "GET", url)
  return response
