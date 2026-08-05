op babelforce-refresh-integration-token(id: String, tokenId: String) -> Any
  description "Refresh an integration token"
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/integrations/{id}/tokens/{tokenId}/refresh")
  response = http.request(method: "PUT", url)
  return response
