op babelforce-clone-application(id: String) -> Any
  description "Clone an application"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/applications/{id}/clone")
  response = http.request(method: "POST", url)
  return response
