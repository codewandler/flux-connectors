op babelforce-delete-integration-token(id: String, tokenId: String) -> Any
  description "Delete an integration token"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/integrations/{id}/tokens/{tokenId}")
  response = http.request(method: "DELETE", url)
  return response
