op babelforce-get-trigger-uses(id: String) -> Any
  description "List a trigger's references"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/triggers/{id}/uses")
  response = http.request(method: "GET", url)
  return response
