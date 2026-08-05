op babelforce-delete-routing(id: String) -> Any
  description "Delete Routing"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/routings/{id}")
  response = http.request(method: "DELETE", url)
  return response
