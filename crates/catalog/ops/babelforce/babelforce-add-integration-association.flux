op babelforce-add-integration-association(integrationId: String, associationId: String, actionName: String) -> Any
  description "Add an integration association"
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/integrations/{integrationId}/association/{associationId}/{actionName}")
  response = http.request(method: "POST", url)
  return response
