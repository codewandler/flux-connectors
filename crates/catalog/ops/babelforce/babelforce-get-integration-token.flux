op babelforce-get-integration-token(id: String, tokenId: String) -> Any
  description "Get an integration token"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/integrations/{id}/tokens/{tokenId}")
  response = http.request(method: "GET", url)
  return response
