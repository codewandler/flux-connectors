op babelforce-clone-integration(id: String) -> Any
  description "Clone an integration"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/integrations/{id}/clone")
  response = http.request(method: "POST", url)
  return response
